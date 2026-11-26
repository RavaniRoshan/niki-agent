//! Bounded command execution and event-aware verdict reduction.
//!
//! Every matching handler runs, in configuration order. Each one is a child
//! process started through `/bin/sh -c` with the payload on stdin; stdout and
//! stderr go to temporary files, so a hook that floods a stream can never
//! deadlock the parent. A child that overruns its handler timeout is killed and
//! reported as a warning — the turn carries on unless a handler returned a
//! blocking verdict.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Map, Value};

use crate::Event;
use crate::config::Handler;

/// Most bytes kept from each of stdout and stderr.
pub const MAX_OUTPUT_BYTES: usize = 100_000;
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// What the hooks for one event decided.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// For `PreToolUse`, the text the tool returns to the model; for
    /// `UserPromptSubmit`, why the prompt was stopped. `None` means continue.
    pub blocked: Option<String>,
    /// Text added to the tool result, or inserted beside the prompt.
    pub context: Vec<String>,
    /// Messages the hook asked to show the person (`systemMessage`).
    pub notices: Vec<String>,
    /// Non-fatal problems: a command that could not start, timed out, or wrote
    /// output this build does not understand.
    pub warnings: Vec<String>,
}

impl Outcome {
    /// Whether the event was stopped.
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        self.blocked.is_some()
    }
}

pub(crate) fn run_handler(
    event: Event,
    handler: &Handler,
    payload: &Value,
    cwd: &Path,
    wire_name: &str,
    outcome: &mut Outcome,
) {
    let bytes = match serde_json::to_vec(payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            outcome
                .warnings
                .push(format!("could not encode the hook payload: {error}"));
            return;
        }
    };

    let mut stdin = match tempfile::tempfile() {
        Ok(file) => file,
        Err(error) => {
            outcome.warnings.push(format!(
                "could not create a temporary file for the hook: {error}"
            ));
            return;
        }
    };
    if let Err(error) = stdin.write_all(&bytes).and_then(|()| stdin.rewind()) {
        outcome
            .warnings
            .push(format!("could not write the hook payload: {error}"));
        return;
    }

    let stdout = match tempfile::tempfile() {
        Ok(file) => file,
        Err(error) => {
            outcome.warnings.push(format!(
                "could not create a temporary file for the hook: {error}"
            ));
            return;
        }
    };
    let stderr = match tempfile::tempfile() {
        Ok(file) => file,
        Err(error) => {
            outcome.warnings.push(format!(
                "could not create a temporary file for the hook: {error}"
            ));
            return;
        }
    };
    let (stdout_child, stderr_child) = match (stdout.try_clone(), stderr.try_clone()) {
        (Ok(stdout), Ok(stderr)) => (stdout, stderr),
        _ => {
            outcome
                .warnings
                .push("could not duplicate a temporary file for the hook".to_owned());
            return;
        }
    };

    let child = Command::new("/bin/sh")
        .arg("-c")
        .arg(&handler.command)
        .current_dir(cwd)
        .stdin(Stdio::from(stdin))
        .stdout(Stdio::from(stdout_child))
        .stderr(Stdio::from(stderr_child))
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(error) => {
            outcome
                .warnings
                .push(format!("{} hook could not start: {error}", event.name()));
            return;
        }
    };

    let status = match wait_with_timeout(&mut child, handler.timeout) {
        Waited::Exited(status) => status,
        Waited::TimedOut => {
            outcome.warnings.push(format!(
                "{} hook exceeded its {}s timeout",
                event.name(),
                seconds(handler.timeout)
            ));
            return;
        }
        Waited::Failed(error) => {
            outcome.warnings.push(format!(
                "{} hook could not be waited for: {error}",
                event.name()
            ));
            return;
        }
    };

    let stdout = match read_bounded(stdout) {
        Ok((text, truncated)) => {
            if truncated {
                outcome.warnings.push(format!(
                    "{} hook stdout exceeded {MAX_OUTPUT_BYTES} bytes and was truncated",
                    event.name()
                ));
            }
            text
        }
        Err(error) => {
            outcome.warnings.push(format!(
                "could not read {} hook stdout: {error}",
                event.name()
            ));
            return;
        }
    };
    let stderr = match read_bounded(stderr) {
        Ok((text, truncated)) => {
            if truncated {
                outcome.warnings.push(format!(
                    "{} hook stderr exceeded {MAX_OUTPUT_BYTES} bytes and was truncated",
                    event.name()
                ));
            }
            text
        }
        Err(error) => {
            outcome.warnings.push(format!(
                "could not read {} hook stderr: {error}",
                event.name()
            ));
            return;
        }
    };

    for effect in interpret(event, status, &stdout, &stderr) {
        apply(event, effect, wire_name, outcome);
    }
}

enum Waited {
    Exited(ExitStatus),
    TimedOut,
    Failed(String),
}

fn wait_with_timeout(child: &mut std::process::Child, timeout: Duration) -> Waited {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Waited::Exited(status),
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Waited::Failed(error.to_string());
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Waited::TimedOut;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

fn read_bounded(mut file: std::fs::File) -> std::io::Result<(String, bool)> {
    file.seek(SeekFrom::Start(0))?;
    let mut buffer = Vec::new();
    file.take((MAX_OUTPUT_BYTES + 1) as u64)
        .read_to_end(&mut buffer)?;
    let truncated = buffer.len() > MAX_OUTPUT_BYTES;
    buffer.truncate(MAX_OUTPUT_BYTES);
    Ok((String::from_utf8_lossy(&buffer).into_owned(), truncated))
}

fn seconds(duration: Duration) -> f64 {
    duration.as_secs_f64()
}

/// One effect a handler asked for, before it is merged into the event outcome.
enum Effect {
    Block(String),
    Context(String),
    Notice(String),
    Warning(String),
}

fn interpret(event: Event, status: ExitStatus, stdout: &str, stderr: &str) -> Vec<Effect> {
    let mut effects = Vec::new();
    match status.code() {
        Some(2) => {
            let reason = stderr.trim();
            effects.push(Effect::Block(if reason.is_empty() {
                "Hook blocked the operation".to_owned()
            } else {
                reason.to_owned()
            }));
            return effects;
        }
        Some(0) => {}
        Some(code) => {
            let detail = stderr.lines().next().unwrap_or("").trim();
            let suffix = if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            };
            effects.push(Effect::Warning(format!(
                "{} hook exited with status {code}{suffix}",
                event.name()
            )));
            return effects;
        }
        None => {
            effects.push(Effect::Warning(format!(
                "{} hook was terminated by a signal",
                event.name()
            )));
            return effects;
        }
    }

    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return effects;
    }

    let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
        if event == Event::UserPromptSubmit {
            effects.push(Effect::Context(trimmed.to_owned()));
        } else {
            effects.push(Effect::Warning(format!(
                "{} hook output is not valid JSON",
                event.name()
            )));
        }
        return effects;
    };
    let Some(object) = value.as_object() else {
        effects.push(Effect::Warning(format!(
            "{} hook output must be a JSON object",
            event.name()
        )));
        return effects;
    };

    let suppress_output = object
        .get("suppressOutput")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if let Some(message) = object.get("systemMessage").and_then(Value::as_str) {
        if !suppress_output {
            effects.push(Effect::Notice(message.to_owned()));
        }
    }

    if let Some(decision) = object.get("decision").and_then(Value::as_str) {
        if decision == "block" {
            effects.push(Effect::Block(block_reason(object)));
        } else {
            effects.push(Effect::Warning(format!(
                "{} hook output decision {decision:?} is not supported and was ignored",
                event.name()
            )));
        }
    }

    if object.get("continue").and_then(Value::as_bool) == Some(false) {
        let reason = object
            .get("stopReason")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        match event {
            Event::PreToolUse => effects.push(Effect::Block(if reason.is_empty() {
                "Blocked by PreToolUse hook".to_owned()
            } else {
                reason.to_owned()
            })),
            Event::UserPromptSubmit => effects.push(Effect::Block(if reason.is_empty() {
                "User prompt submission stopped by hook".to_owned()
            } else {
                reason.to_owned()
            })),
            Event::PostToolUse => {
                if !reason.is_empty() {
                    effects.push(Effect::Context(reason.to_owned()));
                }
            }
        }
    }

    if let Some(specific) = object.get("hookSpecificOutput").and_then(Value::as_object) {
        interpret_specific(event, specific, &mut effects);
    }

    effects
}

fn interpret_specific(event: Event, specific: &Map<String, Value>, effects: &mut Vec<Effect>) {
    match specific.get("hookEventName").and_then(Value::as_str) {
        Some(name) if name == event.name() => {}
        Some(name) => {
            effects.push(Effect::Warning(format!(
                "{} hook hookSpecificOutput.hookEventName is {name:?}; ignored",
                event.name()
            )));
            return;
        }
        None => return,
    }

    if let Some(context) = specific.get("additionalContext").and_then(Value::as_str) {
        if !context.trim().is_empty() {
            effects.push(Effect::Context(context.to_owned()));
        }
    }

    match event {
        Event::PreToolUse => {
            if specific.get("updatedInput").is_some() {
                effects.push(Effect::Warning(
                    "PreToolUse updatedInput is not supported and was ignored".to_owned(),
                ));
            }
            if let Some(decision) = specific.get("permissionDecision").and_then(Value::as_str) {
                match decision {
                    "deny" => {
                        let reason = specific
                            .get("permissionDecisionReason")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .trim();
                        effects.push(Effect::Block(if reason.is_empty() {
                            "Blocked by PreToolUse hook".to_owned()
                        } else {
                            reason.to_owned()
                        }));
                    }
                    "allow" | "ask" | "defer" => effects.push(Effect::Warning(format!(
                        "PreToolUse permissionDecision {decision:?} is not supported and was ignored"
                    ))),
                    other => effects.push(Effect::Warning(format!(
                        "PreToolUse permissionDecision {other:?} is not supported and was ignored"
                    ))),
                }
            }
        }
        Event::PostToolUse => {
            if specific.get("updatedToolOutput").is_some()
                || specific.get("updatedMCPToolOutput").is_some()
            {
                effects.push(Effect::Warning(
                    "PostToolUse output replacement is not supported and was ignored".to_owned(),
                ));
            }
        }
        Event::UserPromptSubmit => {
            if specific
                .get("suppressOriginalPrompt")
                .and_then(Value::as_bool)
                == Some(true)
            {
                effects.push(Effect::Warning(
                    "UserPromptSubmit suppressOriginalPrompt is not supported and was ignored"
                        .to_owned(),
                ));
            }
        }
    }
}

fn block_reason(object: &Map<String, Value>) -> String {
    object
        .get("reason")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .unwrap_or("Blocked by hook")
        .to_owned()
}

fn apply(event: Event, effect: Effect, wire_name: &str, outcome: &mut Outcome) {
    match effect {
        Effect::Block(reason) => match event {
            Event::PreToolUse => {
                if outcome.blocked.is_none() {
                    outcome.blocked = Some(format!("{wire_name} blocked by hook: {reason}"));
                }
            }
            Event::UserPromptSubmit => {
                if outcome.blocked.is_none() {
                    outcome.blocked = Some(reason);
                }
            }
            Event::PostToolUse => outcome.context.push(reason),
        },
        Effect::Context(context) => outcome.context.push(context),
        Effect::Notice(message) => outcome.notices.push(message),
        Effect::Warning(warning) => outcome.warnings.push(warning),
    }
}
