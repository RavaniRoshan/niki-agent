//! The worker thread that runs turns while the screen keeps drawing.
//!
//! A turn is async and long; the screen must stay responsive, keep its scroll
//! position and answer approvals. So the agent lives on its own thread with its
//! own runtime, the screen sends it prompts, and both sides exchange messages
//! over `std::sync::mpsc` channels. Nothing here needs a terminal, so the whole
//! path is testable with a scripted provider (RULES.md 8).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use futures_util::future::BoxFuture;
use niki_agent::{
    Agent, AgentEvent, Approval, ApprovalRequest, Approver, CancelHandle, Compaction, ContextAudit,
    ContextReport, Provider, RunOutcome, SessionUsage, Steering,
};
use niki_policy::Mode;
use niki_tools::{AskAnswer, AskError, AskRequest, Asker, Shell, Tool, ToolContext};
use serde_json::json;
use tokio::sync::oneshot;

/// Builds a provider for one model spec, used to summarise history.
type ProviderBuilder = Box<dyn FnMut(&str) -> Result<Box<dyn Provider>, String> + Send>;

/// Everything a factory needs to build an [`Agent`] for one model.
pub struct SessionParts {
    /// Model spec, `provider:model`.
    pub model: String,
    /// Approval mode in force when the agent was built.
    pub mode: Mode,
    /// The callback that answers a gated command.
    pub approver: Approver,
    /// The handle the screen raises to stop the running turn.
    pub cancel: CancelHandle,
}

/// What the worker tells the screen.
pub enum WorkerEvent {
    /// Something happened during the turn.
    Agent(AgentEvent),
    /// A command needs the person's answer before it can run.
    Approval {
        /// The command and the mode that asked.
        request: ApprovalRequest,
        /// Where the answer goes.
        reply: oneshot::Sender<Approval>,
    },
    /// `request_user_input` asked the person a structured question.
    Question {
        /// The question and its choices.
        request: AskRequest,
        /// Where the answer goes. Dropping it cancels the question.
        reply: mpsc::Sender<Result<AskAnswer, AskError>>,
    },
    /// The turn is over. The message is the failure, if it failed.
    Finished(Result<RunOutcome, String>),
    /// The agent's context report and session usage.
    Context {
        /// Estimated conversation size and the last reported usage.
        report: ContextReport,
        /// Tokens and steps accumulated over the session.
        usage: SessionUsage,
        /// Whether `/context` asked for this and it should be printed.
        announce: bool,
    },
    /// The split of what the next request would inject, for
    /// `/context-doctor`.
    Audit(niki_agent::ContextAudit),
    /// A `/compact` finished, through the provider or without one.
    Compacted {
        /// What the agent replaced, or the provider failure.
        result: Result<Compaction, String>,
        /// Whether the fallback dropped history without a model.
        fallback: bool,
    },
}

impl std::fmt::Debug for WorkerEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Agent(event) => f.debug_tuple("Agent").field(event).finish(),
            Self::Approval { request, .. } => f.debug_tuple("Approval").field(request).finish(),
            Self::Question { request, .. } => f.debug_tuple("Question").field(request).finish(),
            Self::Finished(result) => f
                .debug_tuple("Finished")
                .field(&result.as_ref().map(|_| "ok").map_err(String::as_str))
                .finish(),
            Self::Context {
                report,
                usage,
                announce,
            } => f
                .debug_struct("Context")
                .field("report", report)
                .field("usage", usage)
                .field("announce", announce)
                .finish(),
            Self::Audit(audit) => f.debug_tuple("Audit").field(audit).finish(),
            Self::Compacted { result, fallback } => f
                .debug_struct("Compacted")
                .field(
                    "result",
                    &result.as_ref().map(|_| "ok").map_err(String::as_str),
                )
                .field("fallback", fallback)
                .finish(),
        }
    }
}

/// The screen's handle on the worker thread.
pub struct WorkerHandle {
    requests: Sender<Request>,
    events: Receiver<WorkerEvent>,
    cancel: CancelHandle,
    /// Lines the screen steers into the turn that is running. The worker is
    /// inside `run` while a turn is in flight, so this is shared rather than
    /// sent down the request channel.
    steering: Steering,
    model: String,
}

impl std::fmt::Debug for WorkerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerHandle")
            .field("model", &self.model)
            .field("cancelled", &self.cancel.is_cancelled())
            .finish()
    }
}

enum Request {
    Prompt(String),
    Rewind,
    Shell { command: String, cwd: PathBuf },
    Model(String),
    Mode(Mode),
    Compact(usize),
    Context,
    Audit,
    System(String),
    ClearApprovals,
    Stop,
}

impl WorkerHandle {
    /// Start the worker.
    ///
    /// `build` is called once per model; it runs on the worker thread, so a
    /// provider that needs configuration or a key is built where it is used.
    pub fn spawn<F>(model: &str, build: F) -> Self
    where
        F: FnMut(SessionParts) -> Result<Agent, String> + Send + 'static,
    {
        Self::spawn_with(model, Mode::Manual, build)
    }

    /// Start the worker with an initial approval mode.
    pub fn spawn_with<F>(model: &str, mode: Mode, build: F) -> Self
    where
        F: FnMut(SessionParts) -> Result<Agent, String> + Send + 'static,
    {
        Self::spawn_inner(model, mode, None, build, None)
    }

    /// Start the worker with a provider builder for real compaction.
    ///
    /// Without one, `/compact` falls back to [`Agent::compact_without_model`]
    /// and says so, because the provider moved into the agent and cannot be
    /// borrowed from it.
    pub fn spawn_with_provider<F, P>(model: &str, mode: Mode, build: F, provider: P) -> Self
    where
        F: FnMut(SessionParts) -> Result<Agent, String> + Send + 'static,
        P: FnMut(&str) -> Result<Box<dyn Provider>, String> + Send + 'static,
    {
        Self::spawn_inner(model, mode, None, build, Some(Box::new(provider)))
    }

    /// Start the worker on a runtime the host already built and used.
    ///
    /// A host that opens I/O before the first turn — connecting MCP servers, say
    /// — passes the same runtime here, because a tokio resource belongs to the
    /// runtime that opened it. Every turn then runs where the connection lives.
    pub fn spawn_with_runtime<F>(
        model: &str,
        mode: Mode,
        runtime: tokio::runtime::Runtime,
        build: F,
    ) -> Self
    where
        F: FnMut(SessionParts) -> Result<Agent, String> + Send + 'static,
    {
        Self::spawn_inner(model, mode, Some(runtime), build, None)
    }

    /// Start the worker on the host's runtime, with a provider builder for real
    /// compaction.
    pub fn spawn_with_runtime_and_provider<F, P>(
        model: &str,
        mode: Mode,
        runtime: tokio::runtime::Runtime,
        build: F,
        provider: P,
    ) -> Self
    where
        F: FnMut(SessionParts) -> Result<Agent, String> + Send + 'static,
        P: FnMut(&str) -> Result<Box<dyn Provider>, String> + Send + 'static,
    {
        Self::spawn_inner(model, mode, Some(runtime), build, Some(Box::new(provider)))
    }

    fn spawn_inner<F>(
        model: &str,
        mode: Mode,
        runtime: Option<tokio::runtime::Runtime>,
        mut build: F,
        providers: Option<ProviderBuilder>,
    ) -> Self
    where
        F: FnMut(SessionParts) -> Result<Agent, String> + Send + 'static,
    {
        let (request_tx, request_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let cancel = CancelHandle::new();
        let thread_cancel = cancel.clone();
        let model_label = model.to_owned();
        let mut model = model.to_owned();
        let steering = Steering::new();
        let thread_steering = steering.clone();

        thread::spawn(move || {
            let asker: Arc<dyn Asker + Send + Sync> = Arc::new(ScreenAsker {
                events: event_tx.clone(),
            });
            let Some(runtime) = runtime.or_else(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .ok()
            }) else {
                let _ = event_tx.send(WorkerEvent::Finished(Err(
                    "cannot start the async runtime".to_owned()
                )));
                return;
            };
            let mut agent: Option<Agent> = None;
            let mut providers = providers;
            let mut mode = mode;
            while let Ok(request) = request_rx.recv() {
                match request {
                    Request::Stop => break,
                    Request::Mode(next) => {
                        mode = next;
                        if let Some(agent) = agent.as_mut() {
                            agent.set_mode(next);
                        }
                    }
                    Request::Rewind => {
                        if let Some(agent) = agent.as_mut() {
                            agent.rewind_last_prompt();
                        }
                    }
                    Request::Shell { command, cwd } => {
                        let (output, is_error) = run_shell(&runtime, &cwd, &command);
                        let _ = event_tx.send(WorkerEvent::Agent(AgentEvent::ToolEnd {
                            name: "shell".to_owned(),
                            output,
                            is_error,
                        }));
                    }
                    Request::Model(spec) => {
                        model = spec;
                        // Rebuilt on the next prompt, so a model the provider
                        // does not know fails then, with a prompt to blame.
                        agent = None;
                    }
                    Request::System(system) => {
                        if let Some(agent) = agent.as_mut() {
                            agent.set_system(system);
                        }
                    }
                    Request::ClearApprovals => {
                        if let Some(agent) = agent.as_mut() {
                            agent.clear_session_approvals();
                        }
                    }
                    Request::Prompt(prompt) => {
                        if agent.is_none() {
                            match build(SessionParts {
                                model: model.clone(),
                                mode,
                                approver: approver_for(&event_tx),
                                cancel: thread_cancel.clone(),
                            }) {
                                Ok(built) => {
                                    agent = Some(
                                        built
                                            .with_asker(Arc::clone(&asker))
                                            .with_steering(thread_steering.clone()),
                                    );
                                }
                                Err(message) => {
                                    let _ = event_tx.send(WorkerEvent::Finished(Err(message)));
                                    continue;
                                }
                            }
                        }
                        thread_cancel.reset();
                        let Some(running) = agent.as_mut() else {
                            continue;
                        };
                        let mut forward = |event: &AgentEvent| {
                            let _ = event_tx.send(WorkerEvent::Agent(event.clone()));
                        };
                        let result = runtime.block_on(running.run(prompt, &mut forward));
                        send_context(&event_tx, agent.as_ref());
                        let _ = event_tx.send(WorkerEvent::Finished(
                            result.map_err(|error| error.to_string()),
                        ));
                    }
                    Request::Compact(keep_last) => {
                        let (result, fallback) = match agent.as_mut() {
                            None => (Ok(Compaction::default()), false),
                            Some(running) => match providers.as_mut() {
                                Some(build_provider) => match build_provider(&model) {
                                    Ok(provider) => (
                                        runtime
                                            .block_on(running.compact(provider.as_ref(), keep_last))
                                            .map_err(|error| error.to_string()),
                                        false,
                                    ),
                                    Err(message) => (Err(message), false),
                                },
                                None => (Ok(running.compact_without_model(keep_last)), true),
                            },
                        };
                        send_context(&event_tx, agent.as_ref());
                        let _ = event_tx.send(WorkerEvent::Compacted { result, fallback });
                    }
                    Request::Audit => {
                        let audit = agent
                            .as_ref()
                            .map_or_else(ContextAudit::default, |running| running.context_audit());
                        let _ = event_tx.send(WorkerEvent::Audit(audit));
                    }
                    Request::Context => {
                        let (report, usage) = agent.as_ref().map_or_else(
                            || (ContextReport::default(), SessionUsage::default()),
                            |running| (running.context_report(), running.session_usage()),
                        );
                        let _ = event_tx.send(WorkerEvent::Context {
                            report,
                            usage,
                            announce: true,
                        });
                    }
                }
            }
        });

        Self {
            requests: request_tx,
            events: event_rx,
            cancel,
            steering,
            model: model_label,
        }
    }

    /// The model the worker runs.
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Ask for a turn. Returns false when the worker is gone.
    pub fn submit(&mut self, prompt: impl Into<String>) -> bool {
        self.requests.send(Request::Prompt(prompt.into())).is_ok()
    }

    /// Hand a line to the turn that is running.
    ///
    /// The turn loop appends it to the conversation before its next request, so
    /// the model reads it without a second turn starting. A line handed in while
    /// no turn runs waits for the next one.
    pub fn steer(&self, text: impl Into<String>) {
        self.steering.push(text);
    }

    /// Drop the newest prompt and its answer from the agent's history.
    ///
    /// The view removes the same rows, so leaving the history alone would send
    /// the model an answer with no question.
    pub fn rewind(&self) -> bool {
        self.requests.send(Request::Rewind).is_ok()
    }

    /// Run one `!` line through the shell tool, without involving the model.
    ///
    /// The answer arrives as [`WorkerEvent::Agent`] with a
    /// [`AgentEvent::ToolEnd`], which is the row an ordinary shell call leaves.
    pub fn shell(&self, command: impl Into<String>, cwd: impl Into<PathBuf>) -> bool {
        self.requests
            .send(Request::Shell {
                command: command.into(),
                cwd: cwd.into(),
            })
            .is_ok()
    }

    /// Switch model; the next turn is built for it.
    pub fn select_model(&mut self, spec: impl Into<String>) -> bool {
        self.model = spec.into();
        self.requests
            .send(Request::Model(self.model.clone()))
            .is_ok()
    }

    /// Tell the running agent the approval mode changed.
    pub fn set_mode(&self, mode: Mode) -> bool {
        self.requests.send(Request::Mode(mode)).is_ok()
    }

    /// Ask the agent to summarise the turns older than `keep_last`.
    ///
    /// The worker answers with [`WorkerEvent::Compacted`]; a worker with no
    /// provider builder falls back to a model-free fold and reports it.
    pub fn compact(&self, keep_last: usize) -> bool {
        self.requests.send(Request::Compact(keep_last)).is_ok()
    }

    /// Ask for the agent's [`ContextReport`] and [`SessionUsage`].
    pub fn context(&self) -> bool {
        self.requests.send(Request::Context).is_ok()
    }

    /// Ask for the split of the next request's tokens.
    pub fn audit(&self) -> bool {
        self.requests.send(Request::Audit).is_ok()
    }

    /// Replace the system prompt the running agent sends.
    ///
    /// `/init` calls this after writing a file. A worker with no agent yet
    /// keeps the value in the host's build closure, which recomposes the
    /// prompt when the first turn builds one.
    pub fn set_system(&self, system: impl Into<String>) -> bool {
        self.requests.send(Request::System(system.into())).is_ok()
    }

    /// Drop the "allow for this session" rules the agent remembers.
    ///
    /// `/clear` calls this: the rules are in memory only, and a fresh session
    /// must not inherit them. A worker with no agent yet has none to drop.
    pub fn clear_approvals(&self) -> bool {
        self.requests.send(Request::ClearApprovals).is_ok()
    }

    /// Stop the running turn; the transcript it already produced is kept.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// Take the next message, if one has arrived.
    pub fn try_recv(&self) -> Option<WorkerEvent> {
        self.events.try_recv().ok()
    }

    /// Wait up to `timeout` for the next message.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<WorkerEvent> {
        self.events.recv_timeout(timeout).ok()
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        let _ = self.requests.send(Request::Stop);
    }
}

/// Run one `!` line the way the `shell` tool runs a model's call: same
/// command runner, same 120 s budget, same workspace root.
fn run_shell(runtime: &tokio::runtime::Runtime, cwd: &Path, command: &str) -> (String, bool) {
    let context = match ToolContext::new(cwd) {
        Ok(context) => context,
        Err(error) => return (format!("error: {error}"), true),
    };
    match runtime.block_on(Shell::new().run(json!({ "command": command }), &context)) {
        Ok(output) => (output.text, false),
        Err(error) => (format!("error: {error}"), true),
    }
}

/// Report the agent's context and usage without printing it.
fn send_context(events: &Sender<WorkerEvent>, agent: Option<&Agent>) {
    let (report, usage) = agent.map_or_else(
        || (ContextReport::default(), SessionUsage::default()),
        |agent| (agent.context_report(), agent.session_usage()),
    );
    let _ = events.send(WorkerEvent::Context {
        report,
        usage,
        announce: false,
    });
}

/// The approver the agent calls: it hands the request to the screen and waits
/// for the answer. A screen that has gone away denies, which is the safe answer.
fn approver_for(events: &Sender<WorkerEvent>) -> Approver {
    let events = events.clone();
    Box::new(move |request| {
        let (reply, answer) = oneshot::channel();
        if events
            .send(WorkerEvent::Approval { request, reply })
            .is_err()
        {
            return denied();
        }
        Box::pin(async move { answer.await.unwrap_or(Approval::Deny) })
    })
}

fn denied() -> BoxFuture<'static, Approval> {
    Box::pin(async { Approval::Deny })
}

/// The asker `request_user_input` calls: it hands the question to the screen
/// and blocks until the screen answers or drops the reply channel.
///
/// The tool calls this synchronously inside the turn, so the block is on the
/// worker thread only; the screen's event loop keeps drawing.
struct ScreenAsker {
    events: Sender<WorkerEvent>,
}

impl Asker for ScreenAsker {
    fn ask(&self, request: AskRequest) -> Result<AskAnswer, AskError> {
        let (reply, answer) = mpsc::channel();
        self.events
            .send(WorkerEvent::Question { request, reply })
            .map_err(|_| {
                AskError::Failed("the screen has gone away; ask in prose instead".to_owned())
            })?;
        answer.recv().unwrap_or_else(|_| {
            Err(AskError::Failed(
                "the question was dismissed; ask in prose instead".to_owned(),
            ))
        })
    }
}
