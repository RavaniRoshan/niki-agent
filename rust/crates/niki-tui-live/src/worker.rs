//! The worker thread that runs turns while the screen keeps drawing.
//!
//! A turn is async and long; the screen must stay responsive, keep its scroll
//! position and answer approvals. So the agent lives on its own thread with its
//! own runtime, the screen sends it prompts, and both sides exchange messages
//! over `std::sync::mpsc` channels. Nothing here needs a terminal, so the whole
//! path is testable with a scripted provider (RULES.md 8).

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use futures_util::future::BoxFuture;
use niki_agent::{
    Agent, AgentEvent, Approval, ApprovalRequest, Approver, CancelHandle, RunOutcome,
};
use niki_policy::Mode;
use tokio::sync::oneshot;

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
    /// The turn is over. The message is the failure, if it failed.
    Finished(Result<RunOutcome, String>),
}

impl std::fmt::Debug for WorkerEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Agent(event) => f.debug_tuple("Agent").field(event).finish(),
            Self::Approval { request, .. } => f.debug_tuple("Approval").field(request).finish(),
            Self::Finished(result) => f
                .debug_tuple("Finished")
                .field(&result.as_ref().map(|_| "ok").map_err(String::as_str))
                .finish(),
        }
    }
}

/// The screen's handle on the worker thread.
pub struct WorkerHandle {
    requests: Sender<Request>,
    events: Receiver<WorkerEvent>,
    cancel: CancelHandle,
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
    Model(String),
    Mode(Mode),
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
        Self::spawn_inner(model, mode, None, build)
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
        Self::spawn_inner(model, mode, Some(runtime), build)
    }

    fn spawn_inner<F>(
        model: &str,
        mode: Mode,
        runtime: Option<tokio::runtime::Runtime>,
        mut build: F,
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

        thread::spawn(move || {
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
                    Request::Model(spec) => {
                        model = spec;
                        // Rebuilt on the next prompt, so a model the provider
                        // does not know fails then, with a prompt to blame.
                        agent = None;
                    }
                    Request::Prompt(prompt) => {
                        if agent.is_none() {
                            match build(SessionParts {
                                model: model.clone(),
                                mode,
                                approver: approver_for(&event_tx),
                                cancel: thread_cancel.clone(),
                            }) {
                                Ok(built) => agent = Some(built),
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
                        let _ = event_tx.send(WorkerEvent::Finished(
                            result.map_err(|error| error.to_string()),
                        ));
                    }
                }
            }
        });

        Self {
            requests: request_tx,
            events: event_rx,
            cancel,
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
