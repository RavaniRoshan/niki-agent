use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Instant;

use futures_util::StreamExt;
use niki_hooks::{
    HookRunner, Notify, Outcome, PostToolUse, PreToolUse, ToolCall as HookToolCall, TurnEnd,
    UserPrompt,
};
use niki_policy::{AllowList, Decision, Mode, decide};
use niki_provider::openai::ToolCallAssembler;
use niki_provider::{
    ChatRequest, Message, Provider, Role, StopReason, StreamEvent, ToolCall, ToolSpec, Usage,
};
use niki_tools::{Registry, ToolContext};
use serde_json::{Map, Value};

use crate::context::{
    Compaction, ContextAudit, ContextReport, SessionUsage, audit_context, estimate_text_tokens,
    estimate_tokens, keep_from,
};
use crate::{
    AgentError, AgentEvent, Approval, ApprovalRequest, Approver, CancelHandle, RunOutcome, Steering,
};

/// System prompt handed to the model when the caller sets none.
pub const DEFAULT_SYSTEM_PROMPT: &str = "\
You are Niki, a coding agent working directly in the user's workspace. Use the \
tools to inspect and change files. Prefer small, verifiable steps, run the \
project's own checks when they exist, and say what you changed and what you did \
not verify.";

/// Instruction sent with the older history to produce a compaction summary.
const COMPACT_INSTRUCTION: &str = "\
Summarize the conversation so far for another assistant to continue. Keep \
decisions, file paths and open tasks. Do not add commentary.";

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
    hooks: Option<Arc<dyn HookRunner>>,
    notify: Option<NotifyTarget>,
    session_id: Option<String>,
    cancel: CancelHandle,
    steering: Steering,
    messages: Vec<Message>,
    session: SessionUsage,
    last_usage: Usage,
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
            hooks: None,
            notify: None,
            session_id: None,
            cancel: CancelHandle::new(),
            steering: Steering::new(),
            messages: Vec::new(),
            session: SessionUsage::default(),
            last_usage: Usage::default(),
        }
    }

    /// Seed the conversation the next run continues.
    ///
    /// A resumed session supplies the user and assistant turns it already
    /// stored; the prompt passed to [`Agent::run`] is appended to them.
    #[must_use]
    pub fn with_history(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
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

    /// Take steered lines from a screen while a turn is running.
    ///
    /// Whatever arrived is appended to the conversation before the next request
    /// the loop builds, so a person can add a thought without waiting for the
    /// turn to end or starting a second one.
    #[must_use]
    pub fn with_steering(mut self, steering: Steering) -> Self {
        self.steering = steering;
        self
    }

    /// Install lifecycle hooks: `UserPromptSubmit` before a prompt and
    /// `PreToolUse` / `PostToolUse` around every tool call.
    ///
    /// Without a runner the loop behaves exactly as before, which is what the
    /// existing turn tests rely on.
    #[must_use]
    pub fn with_hooks(mut self, hooks: Arc<dyn HookRunner>) -> Self {
        self.hooks = Some(hooks);
        self
    }

    /// Set the session id sent to hooks. Without one the payload omits it.
    #[must_use]
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Run `notify` once when this agent's turn ends.
    ///
    /// `model` is the `provider:model` spec the host runs, which the payload
    /// carries as `model`. A child agent built without this method never
    /// notifies, so a subagent's turn is not announced.
    #[must_use]
    pub fn with_notify(mut self, notify: Arc<Notify>, model: impl Into<String>) -> Self {
        self.notify = Some(NotifyTarget {
            notify,
            model: model.into(),
        });
        self
    }

    /// Switch the approval mode mid-session, which is what the status bar's
    /// Shift+Tab does.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    /// Replace the system prompt without touching the conversation.
    ///
    /// The next request carries it; history, usage and the approved tools stay
    /// as they were. `/init` uses this after writing `AGENTS.md`, so the model
    /// reads the new file without the session restarting.
    pub fn set_system(&mut self, system: impl Into<String>) {
        self.config.system = system.into();
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

    /// Remove the newest user prompt and everything the turn produced after it.
    ///
    /// This is the history half of restoring a prompt to the composer: the view
    /// drops the row, this drops the messages, and leaving it at that would
    /// leave an answer with no question. Returns whether a prompt was removed.
    pub fn rewind_last_prompt(&mut self) -> bool {
        let Some(index) = self
            .messages
            .iter()
            .rposition(|message| message.role == Role::User)
        else {
            return false;
        };
        self.messages.truncate(index);
        true
    }

    /// Token usage accumulated over every run of this agent.
    #[must_use]
    pub fn usage(&self) -> Usage {
        Usage {
            input_tokens: self.session.input_tokens,
            output_tokens: self.session.output_tokens,
        }
    }

    /// Tokens and steps accumulated over every run of this agent.
    #[must_use]
    pub fn session_usage(&self) -> SessionUsage {
        self.session
    }

    /// The conversation's size, the last reported usage, and the session's
    /// step count.
    #[must_use]
    pub fn context_report(&self) -> ContextReport {
        ContextReport {
            messages: self.messages.len(),
            estimated_tokens: estimate_tokens(&self.messages),
            last_input_tokens: self.last_usage.input_tokens,
            last_output_tokens: self.last_usage.output_tokens,
            steps: self.session.steps,
        }
    }

    /// Audit what the next request would inject: the system prompt, the
    /// registry's tool schemas and the conversation, each estimated in tokens.
    ///
    /// The registry audited is the one this agent dispatches through, so the
    /// figures describe the request the turn loop would actually build.
    #[must_use]
    pub fn context_audit(&self) -> ContextAudit {
        audit_context(&self.config.system, &self.messages, &self.registry)
    }

    /// Run one prompt to completion.
    ///
    /// Every event is passed to `on_event` as it happens. A provider failure
    /// ends the run with an error; a tool failure becomes a tool result the
    /// model sees. A cancelled or over-budget run returns a normal outcome with
    /// the transcript kept.
    ///
    /// With a hook runner installed, `UserPromptSubmit` runs before the model
    /// sees the prompt; a blocking verdict returns [`AgentError::HookBlocked`]
    /// and the prompt is not sent. `PreToolUse` and `PostToolUse` run around
    /// every tool call; a blocked call returns the handler's reason as the tool
    /// result, and hook context is appended to the result (or to the prompt).
    /// A hook that cannot run is a [`AgentEvent::Notice`], never an error.
    ///
    /// Lines handed in through [`Agent::with_steering`] while the run is in
    /// flight join the conversation before the next request, and a line that
    /// arrives just as the model stops keeps the turn going for one more step.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Provider`] when the model request or its stream
    /// fails, and [`AgentError::HookBlocked`] when a `UserPromptSubmit` hook
    /// stops the prompt.
    pub async fn run<F>(
        &mut self,
        prompt: impl Into<String>,
        on_event: &mut F,
    ) -> Result<RunOutcome, AgentError>
    where
        F: FnMut(&AgentEvent),
    {
        let prompt = prompt.into();
        let _ = self.drain_steering();
        let prompt_outcome = self.run_prompt_hooks(&prompt, on_event);
        self.messages.push(Message::user(prompt_with_context(
            prompt,
            &prompt_outcome.context,
        )));
        if let Some(reason) = prompt_outcome.blocked {
            return Err(AgentError::HookBlocked { reason });
        }

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
            let _ = self.drain_steering();

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
                let event = match event {
                    Ok(event) => event,
                    Err(source) => {
                        // A failed turn is announced like a finished one, with
                        // `error` as the reason.
                        self.fire_turn_end("error", steps, tool_calls, &last_text);
                        return Err(AgentError::Provider {
                            provider: self.provider.name().to_owned(),
                            source,
                        });
                    }
                };
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
                        self.session.input_tokens += usage.input_tokens;
                        self.session.output_tokens += usage.output_tokens;
                        self.last_usage = usage;
                        on_event(&AgentEvent::Usage(usage));
                    }
                    StreamEvent::Done(reason) => {
                        // The usage chunk follows the finish reason, so the
                        // stream is read to its end before the turn closes.
                        stop_reason = reason;
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
                if self.drain_steering() {
                    continue 'outer;
                }
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
                let run = self.execute(&call).await;
                for notice in &run.notices {
                    on_event(&AgentEvent::Notice(notice.clone()));
                }
                on_event(&AgentEvent::ToolEnd {
                    name: call.name.clone(),
                    output: run.output.clone(),
                    is_error: run.is_error,
                });
                self.messages.push(Message::tool_result(&call, run.output));
                tool_calls += 1;
            }
        }

        // A line steered in as the model stopped has no request to join; keep
        // it in the history so the next turn reads it in the order it was typed.
        let _ = self.drain_steering();
        self.session.steps += steps;
        // A run that was stopped before its first model turn (a prompt blocked
        // by a hook, or a cancel that arrived immediately) is not a finished
        // turn, so it is not announced.
        if steps > 0 {
            self.fire_turn_end(reason_label(&stop), steps, tool_calls, &last_text);
        }
        Ok(RunOutcome {
            text: last_text,
            steps,
            tool_calls,
            stop,
            usage: run_usage,
        })
    }

    /// Fold the turns older than the last `keep_last` into one summary.
    ///
    /// The older messages and a fixed instruction are sent to `provider`, and
    /// their answer replaces them as a single assistant message. The last
    /// `keep_last` turns stay verbatim; a turn starts at a user message, and a
    /// summary a previous compaction left counts as one. When there is nothing
    /// older to fold, or the answer is empty, the history is unchanged and
    /// [`Compaction::default`] is returned. A provider error leaves the history
    /// unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Provider`] when the summarising request or its
    /// stream fails.
    pub async fn compact(
        &mut self,
        provider: &dyn Provider,
        keep_last: usize,
    ) -> Result<Compaction, AgentError> {
        let split = keep_from(&self.messages, keep_last);
        if split == 0 {
            return Ok(Compaction::default());
        }

        let mut request_messages = self.messages[..split].to_vec();
        request_messages.push(Message::user(COMPACT_INSTRUCTION));
        let request = ChatRequest {
            model: self.config.model.clone(),
            system: self.config.system.clone(),
            messages: request_messages,
            tools: Vec::new(),
            max_tokens: self.config.max_tokens,
        };

        let mut summary = String::new();
        let mut stream = provider.stream(request);
        while let Some(event) = stream.next().await {
            match event.map_err(|source| AgentError::Provider {
                provider: provider.name().to_owned(),
                source,
            })? {
                StreamEvent::TextDelta(delta) => summary.push_str(&delta),
                StreamEvent::Usage(usage) => {
                    self.session.input_tokens += usage.input_tokens;
                    self.session.output_tokens += usage.output_tokens;
                    self.last_usage = usage;
                }
                StreamEvent::ToolCallDelta { .. } | StreamEvent::Done(_) => {}
            }
        }

        let summary = summary.trim();
        if summary.is_empty() {
            return Ok(Compaction::default());
        }
        let content = format!("Summary of the earlier conversation:\n\n{summary}");
        let summary_tokens = estimate_text_tokens(&content);
        self.replace_older(split, content);
        Ok(Compaction {
            replaced: split,
            summary_tokens,
        })
    }

    /// Fold the turns older than the last `keep_last` without a model.
    ///
    /// The older messages become one assistant message that records how many
    /// were dropped, so the model still sees that the conversation was
    /// compacted. This is the fallback for a session with no provider to ask.
    /// When there is nothing older to fold the history is unchanged.
    pub fn compact_without_model(&mut self, keep_last: usize) -> Compaction {
        let split = keep_from(&self.messages, keep_last);
        if split == 0 {
            return Compaction::default();
        }
        let content = match split {
            1 => "1 earlier message was dropped to keep the context small; \
                  the conversation continues from here."
                .to_owned(),
            count => format!(
                "{count} earlier messages were dropped to keep the context small; \
                 the conversation continues from here."
            ),
        };
        let summary_tokens = estimate_text_tokens(&content);
        self.replace_older(split, content);
        Compaction {
            replaced: split,
            summary_tokens,
        }
    }

    fn replace_older(&mut self, split: usize, content: String) {
        let mut messages = Vec::with_capacity(self.messages.len() - split + 1);
        messages.push(Message::assistant(content));
        messages.extend_from_slice(&self.messages[split..]);
        self.messages = messages;
    }

    /// Append the lines steered since the last step and say whether any came.
    fn drain_steering(&mut self) -> bool {
        let pending = self.steering.take();
        let came = !pending.is_empty();
        for text in pending {
            self.messages.push(Message::user(text));
        }
        came
    }

    /// Announce a finished turn to the configured notify program, if any.
    fn fire_turn_end(&self, reason: &str, steps: u32, tool_calls: usize, answer: &str) {
        let Some(target) = &self.notify else {
            return;
        };
        target.notify.fire(&TurnEnd {
            session: self.session_id.as_deref(),
            model: &target.model,
            cwd: &self.ctx.root,
            reason,
            steps,
            tool_calls,
            answer,
        });
    }

    /// Run `UserPromptSubmit` and report what it asked for.
    fn run_prompt_hooks<F>(&self, prompt: &str, on_event: &mut F) -> Outcome
    where
        F: FnMut(&AgentEvent),
    {
        let Some(hooks) = &self.hooks else {
            return Outcome::default();
        };
        let outcome = hooks.user_prompt_submit(&UserPrompt {
            prompt,
            cwd: &self.ctx.root,
            session_id: self.session_id.as_deref(),
        });
        announce(&outcome, on_event);
        outcome
    }

    async fn execute(&mut self, call: &ToolCall) -> ToolRun {
        let mut notices = Vec::new();
        if self.registry.get(&call.name).is_none() {
            return ToolRun {
                output: format!("error: there is no tool named {:?}", call.name),
                is_error: true,
                notices,
            };
        }
        let args = match parse_arguments(&call.arguments) {
            Ok(args) => args,
            Err(message) => {
                return ToolRun {
                    output: message,
                    is_error: true,
                    notices,
                };
            }
        };

        let mut context = Vec::new();
        if let Some(hooks) = &self.hooks {
            let outcome = hooks.pre_tool_use(&PreToolUse {
                tool: HookToolCall {
                    name: &call.name,
                    input: &args,
                    id: &call.id,
                },
                cwd: &self.ctx.root,
                session_id: self.session_id.as_deref(),
            });
            notices.extend(notice_lines(&outcome));
            if let Some(message) = outcome.blocked {
                // A blocked call never reaches the tool, so there is no
                // `PostToolUse` for it; the reason is the tool result.
                let mut output = message;
                append_context(&mut output, &outcome.context);
                return ToolRun {
                    output,
                    is_error: true,
                    notices,
                };
            }
            context.extend(outcome.context);
        }

        let started = Instant::now();
        let (mut output, is_error) = match self.gate(&call.name, &args).await {
            Err(refusal) => (refusal, true),
            Ok(()) => {
                // INFALLIBLE: the registry was checked above and nothing
                // between the two lookups can remove a tool.
                let Some(handle) = self.registry.get(&call.name) else {
                    return ToolRun {
                        output: format!("error: there is no tool named {:?}", call.name),
                        is_error: true,
                        notices,
                    };
                };
                match handle.run(args.clone(), &self.ctx).await {
                    Ok(output) => (output.text, false),
                    Err(error) => (format!("error: {error}"), true),
                }
            }
        };
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

        if let Some(hooks) = &self.hooks {
            let response = Value::String(output.clone());
            let outcome = hooks.post_tool_use(&PostToolUse {
                tool: HookToolCall {
                    name: &call.name,
                    input: &args,
                    id: &call.id,
                },
                cwd: &self.ctx.root,
                session_id: self.session_id.as_deref(),
                response: &response,
                duration_ms,
            });
            notices.extend(notice_lines(&outcome));
            context.extend(outcome.context);
        }

        append_context(&mut output, &context);
        ToolRun {
            output,
            is_error,
            notices,
        }
    }

    /// Gate one call against the live mode.
    ///
    /// `shell` keeps its command-level allow-list. Any other tool that declares
    /// [`Tool::requires_approval`](niki_tools::Tool::requires_approval) — the
    /// built-in file mutators and every mutating MCP tool — takes the same
    /// callback: `Always` records the tool name so a later call of the same tool
    /// does not ask again. `Auto` asks as `Manual` does, with one exception: a
    /// built-in file mutator whose target resolves inside the workspace runs
    /// unattended, which is what the mode promises. A [`Decision::Ask`] goes to
    /// the installed [`Approver`] and waits for the person's answer. A run with
    /// no approver — a headless one — reports the request to the model as a
    /// refusal rather than silently allowing it.
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
        if self.mode == Mode::Auto && self.auto_write_inside(name, args) {
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

    /// Whether `Auto` may run this call unattended: the tool is a built-in file
    /// mutator and the path it names resolves inside the workspace. A path that
    /// escapes fails the resolution, so the call falls through to the approver.
    fn auto_write_inside(&self, name: &str, args: &Value) -> bool {
        let mutator = self
            .registry
            .get(name)
            .is_some_and(|tool| tool.auto_allows_workspace_write());
        if !mutator {
            return false;
        }
        ["file_path", "path"]
            .iter()
            .find_map(|key| args.get(key).and_then(Value::as_str))
            .is_some_and(|path| self.ctx.resolve(path).is_ok())
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

/// What one tool call produced.
struct ToolRun {
    output: String,
    is_error: bool,
    notices: Vec<String>,
}

/// The notify program plus the model spec its payload names.
struct NotifyTarget {
    notify: Arc<Notify>,
    model: String,
}

/// Why a turn ended, in the vocabulary the JSON output uses.
fn reason_label(stop: &StopReason) -> &'static str {
    match stop {
        StopReason::EndTurn => "end_turn",
        StopReason::ToolUse => "tool_use",
        StopReason::MaxTokens => "max_tokens",
        StopReason::Other(reason) if reason == "cancelled" => "cancelled",
        StopReason::Other(reason) if reason.starts_with("step limit") => "step_limit",
        StopReason::Other(_) => "other",
    }
}

/// Emit a hook outcome's user-visible messages. A warning is prefixed so it
/// reads as one in the transcript; a hook's own `systemMessage` is kept as
/// written.
fn announce<F>(outcome: &Outcome, on_event: &mut F)
where
    F: FnMut(&AgentEvent),
{
    for warning in &outcome.warnings {
        on_event(&AgentEvent::Notice(format!("warning: {warning}")));
    }
    for notice in &outcome.notices {
        on_event(&AgentEvent::Notice(notice.clone()));
    }
}

fn notice_lines(outcome: &Outcome) -> Vec<String> {
    let mut lines: Vec<String> = outcome
        .warnings
        .iter()
        .map(|warning| format!("warning: {warning}"))
        .collect();
    lines.extend(outcome.notices.iter().cloned());
    lines
}

/// Attach `UserPromptSubmit` context to the prompt. This transcript has no
/// separate system turn, so the text travels with the prompt instead.
fn prompt_with_context(prompt: String, context: &[String]) -> String {
    if context.is_empty() {
        return prompt;
    }
    format!("{prompt}\n\n{}", context.join("\n\n"))
}

fn append_context(output: &mut String, context: &[String]) {
    if context.is_empty() {
        return;
    }
    if !output.is_empty() {
        output.push_str("\n\n");
    }
    output.push_str(&context.join("\n\n"));
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
