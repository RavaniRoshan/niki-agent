//! The turn-end notification program: one process per finished turn.
//!
//! A configured `[notify] program` is spawned with the payload on stdin and its
//! arguments passed through unchanged — no shell, so an argument is never
//! expanded or re-split. The turn never waits for it: the process is detached,
//! its exit status is collected on a background thread, and a failure is one
//! warning line on stderr. A program that cannot start is a warning too, never
//! an error, and never changes the caller's exit code.

use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// How much of the final answer the payload carries.
pub const ANSWER_LIMIT: usize = 2000;

/// Attempts to start a program that reports `ETXTBSY`, and the pause between
/// them. Fifty attempts at twenty milliseconds cover a second, which is far
/// longer than a concurrent writer holds a script open.
const BUSY_ATTEMPTS: u32 = 50;
const BUSY_BACKOFF: Duration = Duration::from_millis(20);

/// One finished turn, as the program sees it on stdin.
#[derive(Debug, Clone, Copy)]
pub struct TurnEnd<'a> {
    /// Session identity, when the host has one.
    pub session: Option<&'a str>,
    /// `provider:model` spec the turn ran.
    pub model: &'a str,
    /// Workspace root the turn ran in.
    pub cwd: &'a Path,
    /// Why the turn ended: a stop label, `cancelled` or `error`.
    pub reason: &'a str,
    /// Model turns the run took.
    pub steps: u32,
    /// Tool calls the run made.
    pub tool_calls: usize,
    /// The last assistant text, clipped to [`ANSWER_LIMIT`] characters.
    pub answer: &'a str,
}

/// A configured turn-end program.
///
/// Construct with [`Notify::from_program`]; an absent or blank program yields
/// `None`, so a host can install the notifier unconditionally.
#[derive(Debug)]
pub struct Notify {
    program: PathBuf,
    args: Vec<String>,
    pending: Arc<Pending>,
}

/// Outstanding background waits, so a host can flush warnings before exiting.
#[derive(Debug, Default)]
struct Pending {
    count: Mutex<usize>,
    done: Condvar,
}

impl Pending {
    fn begin(&self) {
        let mut count = self.count.lock().unwrap_or_else(PoisonError::into_inner);
        *count += 1;
    }

    fn finish(&self) {
        let mut count = self.count.lock().unwrap_or_else(PoisonError::into_inner);
        *count = count.saturating_sub(1);
        self.done.notify_all();
    }
}

impl Notify {
    /// Build the notifier from a configured program and its arguments.
    ///
    /// Returns `None` when no program is configured, which is the same as
    /// spawning nothing.
    #[must_use]
    pub fn from_program(program: Option<&str>, args: Vec<String>) -> Option<Self> {
        let program = program?.trim();
        if program.is_empty() {
            return None;
        }
        Some(Self {
            program: PathBuf::from(program),
            args,
            pending: Arc::default(),
        })
    }

    /// Run the program once for a finished turn.
    ///
    /// The call writes the payload to a file the child reads as stdin, starts
    /// the child detached, and returns; a background thread waits for it only
    /// to warn about a non-zero exit. Nothing here blocks the turn.
    pub fn fire(&self, turn: &TurnEnd<'_>) {
        let payload = match serde_json::to_vec(&turn_payload(turn)) {
            Ok(payload) => payload,
            Err(error) => {
                warn(format!("could not encode the notify payload: {error}"));
                return;
            }
        };
        let mut stdin = match tempfile::tempfile() {
            Ok(file) => file,
            Err(error) => {
                warn(format!(
                    "the notify program {} could not start: {error}",
                    self.program.display()
                ));
                return;
            }
        };
        if let Err(error) = stdin.write_all(&payload).and_then(|()| stdin.rewind()) {
            warn(format!(
                "the notify program {} could not start: {error}",
                self.program.display()
            ));
            return;
        }

        self.pending.begin();
        let mut attempts = 0u32;
        let mut child = loop {
            // Every attempt needs its own descriptor positioned at the start of
            // the payload: a spawned child consumes the handle it was given.
            let input = match stdin.try_clone().and_then(|mut file| {
                file.rewind()?;
                Ok(file)
            }) {
                Ok(file) => file,
                Err(error) => {
                    self.pending.finish();
                    warn(format!(
                        "the notify program {} could not start: {error}",
                        self.program.display()
                    ));
                    return;
                }
            };
            let spawned = Command::new(&self.program)
                .args(&self.args)
                .current_dir(turn.cwd)
                .stdin(Stdio::from(input))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            match spawned {
                Ok(child) => break child,
                // A script written moments ago is briefly unexecutable: some
                // other process still holds a write descriptor on it, and
                // `exec` answers `ETXTBSY`. The holder is usually a shell that
                // just created the file, or a fork that copied the descriptor;
                // both are gone within milliseconds.
                Err(error)
                    if error.kind() == std::io::ErrorKind::ExecutableFileBusy
                        && attempts < BUSY_ATTEMPTS =>
                {
                    attempts += 1;
                    std::thread::sleep(BUSY_BACKOFF);
                }
                Err(error) => {
                    self.pending.finish();
                    warn(format!(
                        "the notify program {} could not start: {error}",
                        self.program.display()
                    ));
                    return;
                }
            }
        };

        let program = self.program.clone();
        let pending = Arc::clone(&self.pending);
        let spawned = std::thread::Builder::new()
            .name("niki-notify".to_owned())
            .spawn(move || {
                match child.wait() {
                    Ok(status) if status.success() => {}
                    Ok(status) => warn(format!(
                        "the notify program {} exited with {status}",
                        program.display()
                    )),
                    Err(error) => warn(format!(
                        "could not wait for the notify program {}: {error}",
                        program.display()
                    )),
                }
                pending.finish();
            });
        if let Err(error) = spawned {
            self.pending.finish();
            warn(format!(
                "could not track the notify program {}: {error}",
                self.program.display()
            ));
        }
    }

    /// Wait up to `timeout` for outstanding background waits to finish.
    ///
    /// Returns whether all of them finished. A host that is about to exit calls
    /// this so a fast failure is warned about before stderr closes; a slow
    /// notifier is left behind rather than delaying the exit.
    #[must_use]
    pub fn wait_for(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut count = self
            .pending
            .count
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        while *count > 0 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let (next, _) = self
                .pending
                .done
                .wait_timeout(count, remaining)
                .unwrap_or_else(PoisonError::into_inner);
            count = next;
        }
        true
    }
}

fn turn_payload(turn: &TurnEnd<'_>) -> Value {
    json!({
        "event": "turn_end",
        "session": turn.session.unwrap_or(""),
        "model": turn.model,
        "cwd": turn.cwd.display().to_string(),
        "reason": turn.reason,
        "steps": turn.steps,
        "tool_calls": turn.tool_calls,
        "answer": clip(turn.answer),
    })
}

/// The first [`ANSWER_LIMIT`] characters of the answer.
fn clip(answer: &str) -> String {
    answer.chars().take(ANSWER_LIMIT).collect()
}

/// One warning line; control characters cannot break it.
fn warn(message: String) {
    let line: String = message
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    eprintln!("niki: warning: {line}");
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn script(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("notify.sh");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("make executable");
        path
    }

    fn write_script(dir: &Path) -> PathBuf {
        script(
            dir,
            "out=\"$1\"; shift\ncat > \"$out\"\nprintf '%s\\n' \"$@\" > \"$out.args\"",
        )
    }

    #[test]
    fn an_absent_or_blank_program_builds_nothing() {
        assert!(Notify::from_program(None, vec!["--x".to_owned()]).is_none());
        assert!(Notify::from_program(Some(""), Vec::new()).is_none());
        assert!(Notify::from_program(Some("   "), Vec::new()).is_none());
        assert!(Notify::from_program(Some("/bin/true"), Vec::new()).is_some());
    }

    #[test]
    fn the_program_receives_the_documented_payload() {
        let dir = tempfile::tempdir().expect("tempdir");
        let out = dir.path().join("payload.json");
        let notify = Notify::from_program(
            Some(&write_script(dir.path()).display().to_string()),
            vec![out.display().to_string()],
        )
        .expect("notifier");

        notify.fire(&TurnEnd {
            session: Some("session-1"),
            model: "mock:model",
            cwd: dir.path(),
            reason: "end_turn",
            steps: 2,
            tool_calls: 3,
            answer: "all done",
        });

        assert!(notify.wait_for(Duration::from_secs(10)), "the program ran");
        let payload: Value =
            serde_json::from_str(&std::fs::read_to_string(&out).expect("payload")).expect("json");
        assert_eq!(payload["event"], "turn_end");
        assert_eq!(payload["session"], "session-1");
        assert_eq!(payload["model"], "mock:model");
        assert_eq!(payload["cwd"], dir.path().display().to_string());
        assert_eq!(payload["reason"], "end_turn");
        assert_eq!(payload["steps"], 2);
        assert_eq!(payload["tool_calls"], 3);
        assert_eq!(payload["answer"], "all done");
    }

    #[test]
    fn an_absent_session_is_the_empty_string() {
        let dir = tempfile::tempdir().expect("tempdir");
        let out = dir.path().join("payload.json");
        let notify = Notify::from_program(
            Some(&write_script(dir.path()).display().to_string()),
            vec![out.display().to_string()],
        )
        .expect("notifier");

        notify.fire(&TurnEnd {
            session: None,
            model: "mock:model",
            cwd: dir.path(),
            reason: "error",
            steps: 1,
            tool_calls: 0,
            answer: "",
        });

        assert!(notify.wait_for(Duration::from_secs(10)));
        let payload: Value =
            serde_json::from_str(&std::fs::read_to_string(&out).expect("payload")).expect("json");
        assert_eq!(payload["session"], "");
        assert_eq!(payload["reason"], "error");
    }

    #[test]
    fn a_script_held_open_for_writing_is_retried_rather_than_dropped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let out = dir.path().join("payload.json");
        let program = write_script(dir.path());
        // A live write descriptor on a script makes `exec` answer `ETXTBSY`,
        // which is what a shell that just created the file leaves behind.
        let held = std::fs::OpenOptions::new()
            .append(true)
            .open(&program)
            .expect("open for writing");
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            drop(held);
        });

        let notify = Notify::from_program(
            Some(&program.display().to_string()),
            vec![out.display().to_string()],
        )
        .expect("notifier");
        notify.fire(&TurnEnd {
            session: None,
            model: "mock:model",
            cwd: dir.path(),
            reason: "end_turn",
            steps: 1,
            tool_calls: 0,
            answer: "",
        });

        assert!(notify.wait_for(Duration::from_secs(10)), "the program ran");
        writer.join().expect("writer");
        let payload: Value =
            serde_json::from_str(&std::fs::read_to_string(&out).expect("payload")).expect("json");
        assert_eq!(payload["reason"], "end_turn");
    }

    #[test]
    fn arguments_arrive_verbatim_and_are_not_shell_interpreted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let out = dir.path().join("payload.json");
        let notify = Notify::from_program(
            Some(&write_script(dir.path()).display().to_string()),
            vec![
                out.display().to_string(),
                "--title".to_owned(),
                "$(echo pwned)".to_owned(),
                "two words".to_owned(),
            ],
        )
        .expect("notifier");

        notify.fire(&TurnEnd {
            session: Some("s"),
            model: "mock:model",
            cwd: dir.path(),
            reason: "end_turn",
            steps: 1,
            tool_calls: 0,
            answer: "ok",
        });

        assert!(notify.wait_for(Duration::from_secs(10)));
        let args = std::fs::read_to_string(out.with_extension("json.args")).expect("args");
        assert_eq!(
            args.lines().collect::<Vec<_>>(),
            vec!["--title", "$(echo pwned)", "two words"]
        );
        assert!(!dir.path().join("pwned").exists(), "no shell expansion ran");
    }

    #[test]
    fn the_answer_is_clipped_to_the_documented_length() {
        let dir = tempfile::tempdir().expect("tempdir");
        let out = dir.path().join("payload.json");
        let notify = Notify::from_program(
            Some(&write_script(dir.path()).display().to_string()),
            vec![out.display().to_string()],
        )
        .expect("notifier");
        let answer = "é".repeat(ANSWER_LIMIT + 500);

        notify.fire(&TurnEnd {
            session: Some("s"),
            model: "mock:model",
            cwd: dir.path(),
            reason: "end_turn",
            steps: 1,
            tool_calls: 0,
            answer: &answer,
        });

        assert!(notify.wait_for(Duration::from_secs(10)));
        let payload: Value =
            serde_json::from_str(&std::fs::read_to_string(&out).expect("payload")).expect("json");
        let clipped = payload["answer"].as_str().expect("answer");
        assert_eq!(clipped.chars().count(), ANSWER_LIMIT);
        assert!(clipped.chars().all(|character| character == 'é'));
    }

    #[test]
    fn a_missing_program_is_a_warning_not_a_panic() {
        let notify = Notify::from_program(Some("/nonexistent/niki-notify-test"), Vec::new())
            .expect("config");
        notify.fire(&TurnEnd {
            session: None,
            model: "mock:model",
            cwd: Path::new("."),
            reason: "end_turn",
            steps: 1,
            tool_calls: 0,
            answer: "",
        });
        assert!(
            notify.wait_for(Duration::from_secs(1)),
            "nothing to wait for"
        );
    }
}
