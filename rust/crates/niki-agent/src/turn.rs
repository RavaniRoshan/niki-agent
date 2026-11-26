use std::collections::BTreeSet;

use futures_util::StreamExt;
use niki_policy::{AllowList, Decision, Mode, decide};
use niki_provider::openai::ToolCallAssembler;
use niki_provider::{
    ChatRequest, Message, Provider, Role, StopReason, StreamEvent, ToolCall, ToolSpec, Usage,
};
use niki_tools::{Registry, ToolContext};
use serde_json::{Map, Value};

use crate::{
    AgentError, AgentEvent, Approval, ApprovalRequest, Approver, CancelHandle, RunOutcome,
};

/// System prompt handed to the model when the caller sets none.
pub const DEFAULT_SYSTEM_PROMPT: &str = "\
You are Niki, a coding agent working directly in the user's workspace. Use the \
tools to inspect and change files. Prefer small, verifiable steps, run the \
project's own checks when they exist, and say what you changed and what you did \
not verify.";

/// Bounds and identity for one agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentConfig {
    /// Model identifier as the provider names it.
    pub model: String,
    /// System prompt sent with every request.
    pub system: String,
    /// Model turns allowed in one run.
    pub max_steps: u32,
    /// Upper bound on each answer.
    pub max_tokens: u32,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: String::new(),
            system: DEFAULT_SYSTEM_PROMPT.to_owned(),
            max_steps: 40,
            max_tokens: 8_192,
        }
    }
}

/// The turn loop over one provider, workspace and policy.
pub struct Agent {
    provider: Box<dyn Provider>,
    registry: Registry,
    ctx: ToolContext,
    config: AgentConfig,
    mode: Mode,
    allow: AllowList,
    /// Tools the person chose "always allow" for. The shell allow-list does not
    /// apply to them; this is the equivalent record for external tools.
    approved: BTreeSet<String>,
    approver: Option<Approver>,
    cancel: CancelHandle,
    messages: Vec<Message>,
    usage: Usage,
}

impl Agent {
    /// Build an agent. The default policy is [`Mode::Manual`] with an empty
    /// allow-list, so a shell command is refused until the caller opts in with
    /// [`Agent::with_policy`].
    #[must_use]
    pub fn new(provider: Box<dyn Provider>, ctx: ToolContext, config: AgentConfig) -> Self {
        Self {
            provider,
            registry: Registry::new(),
            ctx,
            config,
            mode: Mode::default(),
            allow: AllowList::parse(""),
            approved: BTreeSet::new(),
            approver: None,
            cancel: CancelHandle::new(),
            messages: Vec::new(),
            usage: Usage::default(),
        }
    }

    /// Replace the approval mode and shell allow-list.
    #[must_use]
    pub fn with_policy(mut self, mode: Mode, allow: AllowList) -> Self {
        self.mode = mode;
        self.allow = allow;
        self
    }

    /// Install the callback that answers [`Decision::Ask`].
    ///
    /// Without one the agent has nobody to ask and reports the request to the
    /// model as a refusal.
    #[must_use]
    pub fn with_approver(mut self, approver: Approver) -> Self {
        self.approver = Some(approver);
        self
    }

    /// Switch the approval mode mid-session, which is what the status bar's
    /// Shift+Tab does.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    /// The mode currently in force.
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Replace the tool registry, for tests and for a host that adds tools.
    #[must_use]
    pub fn with_registry(mut self, registry: Registry) -> Self {
        self.registry = registry;
        self
    }

    /// Attach the handle a UI thread cancels.
    #[must_use]
    pub fn with_cancel(mut self, cancel: CancelHandle) -> Self {
        self.cancel = cancel;
        self
    }

    /// The conversation so far: user, assistant and tool messages in order.
    #[must_use]
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Token usage accumulated over every run of this agent.
    #[must_use]
    pub fn usage(&self) -> Usage {
        self.usage
    }

    /// Run one prompt to completion.
    ///
    /// Every event is passed to `on_event` as it happens. A provider failure
    /// ends the run with an error; a tool failure becomes a tool result the
    /// model sees. A cancelled or over-budget run returns a normal outcome with
    /// the transcript kept.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Provider`] when the model request or its stream
    /// fails.
    pub async fn run<F>(
        &mut self,
        prompt: impl Into<String>,
        on_event: &mut F,
    ) -> Result<RunOutcome, AgentError>
    where
        F: FnMut(&AgentEvent),
    {
        self.messages.push(Message::user(prompt));

        let mut steps = 0u32;
        let mut tool_calls = 0usize;
        let mut last_text = String::new();
        let mut run_usage = Usage::default();
        let stop;

        'outer: loop {
            if self.cancel.is_cancelled() {
                on_event(&AgentEvent::Cancelled);
                stop = StopReason::Other("cancelled".to_owned());
                break 'outer;
            }
            if steps >= self.config.max_steps {
                on_event(&AgentEvent::StepLimit { steps });
                stop = StopReason::Other(format!("step limit {steps} reached"));
                break 'outer;
            }
            steps += 1;

            let request = ChatRequest {
                model: self.config.model.clone(),
                system: self.config.system.clone(),
                messages: self.messages.clone(),
                tools: tool_specs(&self.registry),
                max_tokens: self.config.max_tokens,
            };

            let mut turn_text = String::new();
            let mut assembler = ToolCallAssembler::default();
            let mut stop_reason = StopReason::EndTurn;
            let mut stream = self.provider.stream(request);
            while let Some(event) = stream.next().await {
                let event = event.map_err(|source| AgentError::Provider {
                    provider: self.provider.name().to_owned(),
                    source,
                })?;
                match event {
                    StreamEvent::TextDelta(delta) => {
                        turn_text.push_str(&delta);
                        on_event(&AgentEvent::Text(delta));
                    }
                    StreamEvent::ToolCallDelta {
                        index,
                        id,
                        name,
                        arguments,
                    } => assembler.push(index, id, name, &arguments),
                    StreamEvent::Usage(usage) => {
                        run_usage.input_tokens += usage.input_tokens;
                        run_usage.output_tokens += usage.output_tokens;
                        self.usage.input_tokens += usage.input_tokens;
                        self.usage.output_tokens += usage.output_tokens;
                        on_event(&AgentEvent::Usage(usage));
                    }
                    StreamEvent::Done(reason) => {
                        stop_reason = reason;
                        break;
                    }
                }
                if self.cancel.is_cancelled() {
                    on_event(&AgentEvent::Cancelled);
                    stop = StopReason::Other("cancelled".to_owned());
                    break 'outer;
                }
            }

            let calls = assembler.finish();
            self.messages.push(Message {
                role: Role::Assistant,
                content: turn_text.clone(),
                tool_calls: calls.clone(),
                tool_call_id: None,
            });
            last_text = turn_text;

            if calls.is_empty() {
                stop = stop_reason;
                break 'outer;
            }

            for call in calls {
                if self.cancel.is_cancelled() {
                    on_event(&AgentEvent::Cancelled);
                    stop = StopReason::Other("cancelled".to_owned());
                    break 'outer;
                }
                on_event(&AgentEvent::ToolStart {
                    name: call.name.clone(),
                    arguments: call.arguments.clone(),
                });
                let (output, is_error) = self.execute(&call).await;
                on_event(&AgentEvent::ToolEnd {
                    name: call.name.clone(),
                    output: output.clone(),
                    is_error,
                });
                self.messages.push(Message::tool_result(&call, output));
                tool_calls += 1;
            }
        }

        Ok(RunOutcome {
            text: last_text,
            steps,
            tool_calls,
            stop,
            usage: run_usage,
        })
    }

    async fn execute(&mut self, call: &ToolCall) -> (String, bool) {
        if self.registry.get(&call.name).is_none() {
            return (
                format!("error: there is no tool named {:?}", call.name),
                true,
            );
        }
        let args = match parse_arguments(&call.arguments) {
            Ok(args) => args,
            Err(message) => return (message, true),
        };
        if let Err(message) = self.gate(&call.name, &args).await {
            return (message, true);
        }
        // INFALLIBLE: the registry was checked above and nothing between the
        // two lookups can remove a tool.
        let Some(handle) = self.registry.get(&call.name) else {
            return (
                format!("error: there is no tool named {:?}", call.name),
                true,
            );
        };
        match handle.run(args, &self.ctx).await {
            Ok(output) => (output.text, false),
            Err(error) => (format!("error: {error}"), true),
        }
    }

    /// Gate one call against the live mode.
    ///
    /// `shell` keeps its command-level allow-list. Any other tool that declares
    /// [`Tool::requires_approval`](niki_tools::Tool::requires_approval) — every
    /// mutating MCP tool — takes the same callback: `Always` records the tool
    /// name so a later call of the same tool does not ask again, and Auto asks
    /// as Manual does because this client has no classifier. A
    /// [`Decision::Ask`] goes to the installed [`Approver`] and waits for the
    /// person's answer. A run with no approver — a headless one — reports the
    /// request to the model as a refusal rather than silently allowing it.
    async fn gate(&mut self, name: &str, args: &Value) -> Result<(), String> {
        if name == "shell" {
            let Some(command) = args.get("command").and_then(Value::as_str) else {
                return Err("error: shell needs a command string".to_owned());
            };
            return self.gate_command(command).await;
        }

        let gated = self
            .registry
            .get(name)
            .is_some_and(|tool| tool.requires_approval());
        if !gated || self.approved.contains(name) || self.mode == Mode::Yolo {
            return Ok(());
        }

        let request = ApprovalRequest {
            command: describe_call(name, args),
            mode: self.mode,
        };
        match self.ask(request).await {
            None => Err(format!(
                "refused: `{name}` needs approval and this run has nobody to ask (mode: {}). \
                 Approve it in the full-screen app or set the mode to yolo.",
                self.mode.label()
            )),
            Some(Approval::Once) => Ok(()),
            Some(Approval::Always) => {
                self.approved.insert(name.to_owned());
                Ok(())
            }
            Some(Approval::Deny) => Err(format!("refused: `{name}` was not approved")),
        }
    }

    async fn gate_command(&mut self, command: &str) -> Result<(), String> {
        match decide(self.mode, &self.allow, command) {
            Decision::Allow => Ok(()),
            Decision::Ask => {
                let request = ApprovalRequest {
                    command: command.to_owned(),
                    mode: self.mode,
                };
                match self.ask(request).await {
                    None => Err(format!(
                        "refused: `{command}` needs approval and this run has nobody to ask \
                         (mode: {}). Set the mode to auto, add the command to the allow-list, \
                         or run it yourself.",
                        self.mode.label()
                    )),
                    Some(Approval::Once) => Ok(()),
                    Some(Approval::Always) => {
                        self.allow.add(command);
                        Ok(())
                    }
                    Some(Approval::Deny) => Err(format!("refused: `{command}` was not approved")),
                }
            }
            Decision::Deny => Err(format!("refused by policy: `{command}`")),
        }
    }

    /// Put one request to the installed approver, or `None` when there is
    /// nobody to ask.
    async fn ask(&self, request: ApprovalRequest) -> Option<Approval> {
        match self.approver.as_ref() {
            Some(approver) => Some(approver(request).await),
            None => None,
        }
    }
}

/// A one-line description of an external tool call for the approval prompt.
fn describe_call(name: &str, args: &Value) -> String {
    let rendered = serde_json::to_string(args).unwrap_or_else(|_| "{}".to_owned());
    let mut text = format!("{name} {rendered}");
    if text.chars().count() > 160 {
        text = text.chars().take(157).collect::<String>() + "...";
    }
    text
}

/// The tool array for a request, read back from the registry's JSON form.
fn tool_specs(registry: &Registry) -> Vec<ToolSpec> {
    let Value::Array(items) = registry.specs() else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|item| {
            Some(ToolSpec {
                name: item.get("name")?.as_str()?.to_owned(),
                description: item.get("description")?.as_str()?.to_owned(),
                schema: item.get("parameters")?.clone(),
            })
        })
        .collect()
}

/// Parse a tool-call argument string into an object, treating an empty string
/// as no arguments.
fn parse_arguments(raw: &str) -> Result<Value, String> {
    if raw.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    let value: Value = serde_json::from_str(raw)
        .map_err(|error| format!("error: arguments are not valid JSON ({error})"))?;
    if value.is_object() {
        Ok(value)
    } else {
        Err("error: arguments must be a JSON object".to_owned())
    }
}
