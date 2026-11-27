//! A whole turn through the worker thread, against a scripted provider.
//!
//! No model endpoint is contacted (RULES.md 8): the provider replays recorded
//! stream events, and the test drives the same channel the screen reads.

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use futures_util::stream::BoxStream;
use futures_util::{Stream, StreamExt};
use niki_agent::{Agent, AgentConfig, AgentEvent, AllowList};
use niki_policy::Mode;
use niki_provider::{ChatRequest, Provider, ProviderError, StopReason, StreamEvent, Usage};
use niki_tools::ToolContext;
use niki_tui_live::{App, Command, SessionInfo, TurnState, WorkerEvent, WorkerHandle};

/// A provider that replays one scripted turn per request.
struct Scripted {
    turns: Arc<Mutex<VecDeque<Vec<StreamEvent>>>>,
}

impl Provider for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }

    fn stream(
        &self,
        _request: ChatRequest,
    ) -> BoxStream<'static, Result<StreamEvent, ProviderError>> {
        let events = self
            .turns
            .lock()
            .expect("lock")
            .pop_front()
            .unwrap_or_default();
        futures_util::stream::iter(events.into_iter().map(Ok)).boxed()
    }
}

/// A provider whose second event waits until the test lets it through, so a
/// cancel can land while a turn is in flight.
struct Gated {
    prefix: Option<StreamEvent>,
    suffix: Option<StreamEvent>,
    gate: Arc<Mutex<Receiver<()>>>,
    state: u8,
}

impl Stream for Gated {
    type Item = Result<StreamEvent, ProviderError>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match this.state {
            0 => {
                this.state = 1;
                Poll::Ready(this.prefix.take().map(Ok))
            }
            1 => {
                // A test double may block here: the worker runtime has nothing
                // else to run while this turn is the only task.
                let released = this.gate.lock().expect("gate").recv().is_ok();
                this.state = 2;
                if released {
                    Poll::Ready(this.suffix.take().map(Ok))
                } else {
                    Poll::Ready(None)
                }
            }
            _ => Poll::Ready(None),
        }
    }
}

struct GatedProvider {
    gate: Arc<Mutex<Receiver<()>>>,
}

impl Provider for GatedProvider {
    fn name(&self) -> &str {
        "gated"
    }

    fn stream(
        &self,
        _request: ChatRequest,
    ) -> BoxStream<'static, Result<StreamEvent, ProviderError>> {
        let stream = Gated {
            prefix: Some(StreamEvent::TextDelta("half an answer".to_owned())),
            suffix: Some(StreamEvent::TextDelta(", then more".to_owned())),
            gate: Arc::clone(&self.gate),
            state: 0,
        };
        Box::pin(stream)
    }
}

fn text_turn(text: &str) -> Vec<StreamEvent> {
    vec![
        StreamEvent::TextDelta(text.to_owned()),
        StreamEvent::Usage(Usage {
            input_tokens: 12,
            output_tokens: 5,
        }),
        StreamEvent::Done(StopReason::EndTurn),
    ]
}

fn tool_turn(id: &str, name: &str, arguments: &str) -> Vec<StreamEvent> {
    vec![
        StreamEvent::ToolCallDelta {
            index: 0,
            id: Some(id.to_owned()),
            name: Some(name.to_owned()),
            arguments: arguments.to_owned(),
        },
        StreamEvent::Done(StopReason::ToolUse),
    ]
}

fn app(root: &std::path::Path) -> App {
    App::new(SessionInfo {
        model: "mock:mock-model".to_owned(),
        mode: Mode::Manual,
        cwd: root.to_path_buf(),
    })
}

/// Type a prompt into the composer and send it, as the screen does.
fn send(app: &mut App, worker: &mut WorkerHandle, prompt: &str) {
    for character in prompt.chars() {
        let _ = app.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
    }
    match app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)) {
        Some(Command::Submit(text)) => assert!(worker.submit(text)),
        other => panic!("expected a prompt, got {other:?}"),
    }
}

/// Read worker messages until the turn finishes, folding them into `app`.
fn pump(app: &mut App, worker: &WorkerHandle) -> Vec<AgentEvent> {
    let mut seen = Vec::new();
    loop {
        match worker.recv_timeout(Duration::from_secs(10)) {
            Some(WorkerEvent::Agent(event)) => {
                seen.push(event.clone());
                app.apply(event);
            }
            Some(WorkerEvent::Finished(result)) => {
                app.finish(result.map_err(|message| message.to_string()));
                return seen;
            }
            Some(WorkerEvent::Context {
                report,
                usage,
                announce,
            }) => {
                if announce {
                    app.show_context(report, usage);
                } else {
                    app.set_context(report, usage);
                }
            }
            Some(WorkerEvent::Compacted { result, fallback }) => app.compacted(result, fallback),
            Some(WorkerEvent::Approval { .. }) => panic!("this worker installs no approver"),
            None => panic!("the worker went quiet before the turn finished"),
        }
    }
}

#[test]
fn a_prompt_runs_on_the_worker_and_reaches_the_screen() {
    let workspace = tempfile::tempdir().expect("workspace");
    let turns = Arc::new(Mutex::new(VecDeque::from(vec![text_turn(
        "hello from the worker",
    )])));
    let build_turns = Arc::clone(&turns);
    let ctx = ToolContext::new(workspace.path()).expect("workspace");

    let mut worker = WorkerHandle::spawn("mock:mock-model", move |parts| {
        Ok(Agent::new(
            Box::new(Scripted {
                turns: Arc::clone(&build_turns),
            }),
            ctx.clone(),
            AgentConfig {
                model: "mock-model".to_owned(),
                ..AgentConfig::default()
            },
        )
        .with_policy(parts.mode, AllowList::parse(""))
        .with_cancel(parts.cancel))
    });
    let mut app = app(workspace.path());

    send(&mut app, &mut worker, "say hello");
    let events = pump(&mut app, &worker);

    assert!(events.contains(&AgentEvent::Text("hello from the worker".to_owned())));
    let rows = app.transcript.rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].text, "say hello");
    assert_eq!(rows[1].text, "hello from the worker");
    assert_eq!(app.status.tokens.input_tokens, 12);
    assert_eq!(app.status.total_label().as_deref(), Some("17 tok"));
    assert_eq!(app.status.turn, TurnState::Idle);
    assert!(
        turns.lock().expect("lock").is_empty(),
        "one turn per prompt"
    );
}

#[test]
fn compact_summarises_older_turns_through_the_agent() {
    let workspace = tempfile::tempdir().expect("workspace");
    let turns = Arc::new(Mutex::new(VecDeque::from(vec![
        text_turn("first answer"),
        text_turn("second answer"),
    ])));
    let build_turns = Arc::clone(&turns);
    let summary = Arc::new(Mutex::new(VecDeque::from(vec![text_turn("a summary")])));
    let compact_turns = Arc::clone(&summary);
    let ctx = ToolContext::new(workspace.path()).expect("workspace");

    let mut worker = WorkerHandle::spawn_with_provider(
        "mock:mock-model",
        Mode::Manual,
        move |parts| {
            Ok(Agent::new(
                Box::new(Scripted {
                    turns: Arc::clone(&build_turns),
                }),
                ctx.clone(),
                AgentConfig {
                    model: "mock-model".to_owned(),
                    ..AgentConfig::default()
                },
            )
            .with_policy(parts.mode, AllowList::parse(""))
            .with_cancel(parts.cancel))
        },
        move |_spec| {
            Ok(Box::new(Scripted {
                turns: Arc::clone(&compact_turns),
            }) as Box<dyn Provider>)
        },
    );
    let mut app = app(workspace.path());

    send(&mut app, &mut worker, "first");
    pump(&mut app, &worker);
    send(&mut app, &mut worker, "second");
    pump(&mut app, &worker);

    assert!(worker.compact(1));
    loop {
        match worker
            .recv_timeout(Duration::from_secs(10))
            .expect("a compaction result")
        {
            WorkerEvent::Compacted { result, fallback } => {
                assert!(!fallback, "the provider answered the summary");
                let compaction = result.expect("the compaction");
                assert_eq!(compaction.replaced, 2, "the two older messages");
                assert!(compaction.summary_tokens > 0, "{compaction:?}");
                break;
            }
            WorkerEvent::Context { report, .. } => {
                assert_eq!(report.messages, 3, "summary plus the kept turn");
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[test]
fn compact_without_a_provider_falls_back_and_says_so() {
    let workspace = tempfile::tempdir().expect("workspace");
    let turns = Arc::new(Mutex::new(VecDeque::from(vec![
        text_turn("first answer"),
        text_turn("second answer"),
    ])));
    let build_turns = Arc::clone(&turns);
    let ctx = ToolContext::new(workspace.path()).expect("workspace");

    let mut worker = WorkerHandle::spawn("mock:mock-model", move |parts| {
        Ok(Agent::new(
            Box::new(Scripted {
                turns: Arc::clone(&build_turns),
            }),
            ctx.clone(),
            AgentConfig {
                model: "mock-model".to_owned(),
                ..AgentConfig::default()
            },
        )
        .with_policy(parts.mode, AllowList::parse(""))
        .with_cancel(parts.cancel))
    });
    let mut app = app(workspace.path());

    send(&mut app, &mut worker, "first");
    pump(&mut app, &worker);
    send(&mut app, &mut worker, "second");
    pump(&mut app, &worker);

    assert!(worker.compact(1));
    loop {
        match worker
            .recv_timeout(Duration::from_secs(10))
            .expect("a compaction result")
        {
            WorkerEvent::Compacted { result, fallback } => {
                assert!(fallback, "there was no provider to summarise with");
                assert_eq!(result.expect("the fallback compaction").replaced, 2);
                break;
            }
            WorkerEvent::Context { .. } => {}
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[test]
fn a_tool_call_runs_in_the_workspace_and_the_result_reaches_the_screen() {
    let workspace = tempfile::tempdir().expect("workspace");
    let turns = Arc::new(Mutex::new(VecDeque::from(vec![
        tool_turn(
            "call-1",
            "write_file",
            r#"{"path":"note.txt","content":"written by the worker"}"#,
        ),
        text_turn("done"),
    ])));
    let build_turns = Arc::clone(&turns);
    let ctx = ToolContext::new(workspace.path()).expect("workspace");

    let mut worker = WorkerHandle::spawn("mock:mock-model", move |parts| {
        Ok(Agent::new(
            Box::new(Scripted {
                turns: Arc::clone(&build_turns),
            }),
            ctx.clone(),
            AgentConfig {
                model: "mock-model".to_owned(),
                ..AgentConfig::default()
            },
        )
        .with_policy(parts.mode, AllowList::parse(""))
        .with_cancel(parts.cancel))
    });
    let mut app = app(workspace.path());

    send(&mut app, &mut worker, "write note.txt");
    pump(&mut app, &worker);

    assert_eq!(
        std::fs::read_to_string(workspace.path().join("note.txt")).expect("file"),
        "written by the worker"
    );
    let kinds: Vec<_> = app
        .transcript
        .rows()
        .iter()
        .map(|row| (row.kind, row.title.clone()))
        .collect();
    assert_eq!(kinds[0].0, niki_tui_live::RowKind::User);
    assert_eq!(kinds[1].0, niki_tui_live::RowKind::ToolCall);
    assert_eq!(kinds[1].1.as_deref(), Some("write_file"));
    assert_eq!(kinds[2].0, niki_tui_live::RowKind::ToolResult);
    assert_eq!(kinds[3].0, niki_tui_live::RowKind::Assistant);
    assert_eq!(kinds[3].1, None);
    assert_eq!(app.status.turn, TurnState::Idle);
}

#[test]
fn cancelling_keeps_the_text_the_turn_already_wrote() {
    let workspace = tempfile::tempdir().expect("workspace");
    let (release, gate) = mpsc::channel();
    let gate = Arc::new(Mutex::new(gate));
    let build_gate = Arc::clone(&gate);
    let ctx = ToolContext::new(workspace.path()).expect("workspace");

    let mut worker = WorkerHandle::spawn("mock:mock-model", move |parts| {
        Ok(Agent::new(
            Box::new(GatedProvider {
                gate: Arc::clone(&build_gate),
            }),
            ctx.clone(),
            AgentConfig {
                model: "mock-model".to_owned(),
                ..AgentConfig::default()
            },
        )
        .with_policy(parts.mode, AllowList::parse(""))
        .with_cancel(parts.cancel))
    });
    let mut app = app(workspace.path());

    send(&mut app, &mut worker, "write something long");

    // Wait for the first delta, stop the turn, and only then let the provider
    // produce its second event: the loop notices the cancel at its next check.
    let first = worker
        .recv_timeout(Duration::from_secs(10))
        .expect("the first delta");
    match first {
        WorkerEvent::Agent(AgentEvent::Text(text)) => {
            assert_eq!(text, "half an answer");
            app.apply(AgentEvent::Text(text));
        }
        other => panic!("expected answer text, got {other:?}"),
    }
    worker.cancel();
    release.send(()).expect("release the provider");

    let mut saw_cancel = false;
    loop {
        match worker
            .recv_timeout(Duration::from_secs(10))
            .expect("the turn to finish")
        {
            WorkerEvent::Agent(event) => {
                saw_cancel |= event == AgentEvent::Cancelled;
                app.apply(event);
            }
            WorkerEvent::Finished(result) => {
                let outcome = result.expect("a cancelled turn is not a failure").clone();
                assert_eq!(outcome.stop, StopReason::Other("cancelled".to_owned()));
                app.finish(Ok(outcome));
                break;
            }
            WorkerEvent::Context {
                report,
                usage,
                announce,
            } => {
                if announce {
                    app.show_context(report, usage);
                } else {
                    app.set_context(report, usage);
                }
            }
            WorkerEvent::Compacted { result, fallback } => app.compacted(result, fallback),
            WorkerEvent::Approval { .. } => panic!("this worker installs no approver"),
        }
    }

    assert!(saw_cancel);
    assert_eq!(app.status.turn, TurnState::Cancelled);
    let first_row = &app.transcript.rows()[1];
    assert!(
        first_row.text.starts_with("half an answer"),
        "{}",
        first_row.text
    );
}

#[test]
fn a_model_the_provider_cannot_build_is_reported_without_killing_the_screen() {
    let workspace = tempfile::tempdir().expect("workspace");
    let mut worker = WorkerHandle::spawn("mock:missing", |_parts| {
        Err("no provider block named missing".to_owned())
    });
    let mut app = app(workspace.path());

    assert!(worker.submit("anything"));
    let event = worker
        .recv_timeout(Duration::from_secs(10))
        .expect("a failure");
    match event {
        WorkerEvent::Finished(Err(message)) => {
            app.finish(Err(message.clone()));
            assert!(message.contains("missing"), "{message}");
        }
        other => panic!("expected a failure, got {other:?}"),
    }
    assert_eq!(app.status.turn, TurnState::Failed);
    assert!(app.transcript.rows().last().is_some());
    assert_eq!(worker.model(), "mock:missing");
}

/// Drive keys on the screen and pass the answer back over the reply channel,
/// which is what the event loop does.
fn pump_answering(app: &mut App, worker: &WorkerHandle, keys: &[KeyCode]) -> Vec<AgentEvent> {
    let mut seen = Vec::new();
    let mut pending_keys: Vec<KeyCode> = keys.iter().rev().copied().collect();
    loop {
        match worker.recv_timeout(Duration::from_secs(10)) {
            Some(WorkerEvent::Agent(event)) => {
                seen.push(event.clone());
                app.apply(event);
            }
            Some(WorkerEvent::Approval { request, reply }) => {
                app.request_approval(request);
                let key = pending_keys.pop().expect("a key for every question");
                match app.handle_key(KeyEvent::new(key, KeyModifiers::NONE)) {
                    Some(Command::Answer(answer)) => reply.send(answer).expect("answer"),
                    other => panic!("expected an answer, got {other:?}"),
                }
            }
            Some(WorkerEvent::Finished(result)) => {
                app.finish(result.map_err(|message| message.to_string()));
                return seen;
            }
            Some(WorkerEvent::Context {
                report,
                usage,
                announce,
            }) => {
                if announce {
                    app.show_context(report, usage);
                } else {
                    app.set_context(report, usage);
                }
            }
            Some(WorkerEvent::Compacted { result, fallback }) => app.compacted(result, fallback),
            None => panic!("the worker went quiet before the turn finished"),
        }
    }
}

#[test]
fn an_approval_answer_lets_the_gated_command_run() {
    let workspace = tempfile::tempdir().expect("workspace");
    let turns = Arc::new(Mutex::new(VecDeque::from(vec![
        tool_turn(
            "call-1",
            "shell",
            r#"{"command":"echo approved > approved.txt"}"#,
        ),
        text_turn("done"),
    ])));
    let build_turns = Arc::clone(&turns);
    let ctx = ToolContext::new(workspace.path()).expect("workspace");

    let mut worker = WorkerHandle::spawn("mock:mock-model", move |parts| {
        Ok(Agent::new(
            Box::new(Scripted {
                turns: Arc::clone(&build_turns),
            }),
            ctx.clone(),
            AgentConfig {
                model: "mock-model".to_owned(),
                ..AgentConfig::default()
            },
        )
        .with_policy(parts.mode, AllowList::parse(""))
        .with_cancel(parts.cancel)
        .with_approver(parts.approver))
    });
    let mut app = app(workspace.path());

    send(&mut app, &mut worker, "echo it");

    let seen = pump_answering(&mut app, &worker, &[KeyCode::Char('y')]);

    assert_eq!(
        std::fs::read_to_string(workspace.path().join("approved.txt")).expect("file"),
        "approved\n"
    );
    let tool_end = seen
        .iter()
        .find_map(|event| match event {
            AgentEvent::ToolEnd {
                name,
                output,
                is_error,
            } => Some((name.as_str(), output.as_str(), *is_error)),
            _ => None,
        })
        .expect("a tool result");
    assert_eq!(tool_end.0, "shell");
    assert!(!tool_end.2, "the command succeeded: {}", tool_end.1);
    assert!(tool_end.1.contains("exit_code: 0"), "{}", tool_end.1);
    assert!(app.modal.is_none());
    assert!(app.pending.is_none());
    assert_eq!(app.status.turn, TurnState::Idle);
}

#[test]
fn always_answers_the_next_identical_command_without_asking() {
    let workspace = tempfile::tempdir().expect("workspace");
    let turns = Arc::new(Mutex::new(VecDeque::from(vec![
        tool_turn("call-1", "shell", r#"{"command":"echo once > twice.txt"}"#),
        tool_turn("call-2", "shell", r#"{"command":"echo once > twice.txt"}"#),
        text_turn("done"),
    ])));
    let build_turns = Arc::clone(&turns);
    let ctx = ToolContext::new(workspace.path()).expect("workspace");

    let mut worker = WorkerHandle::spawn("mock:mock-model", move |parts| {
        Ok(Agent::new(
            Box::new(Scripted {
                turns: Arc::clone(&build_turns),
            }),
            ctx.clone(),
            AgentConfig {
                model: "mock-model".to_owned(),
                ..AgentConfig::default()
            },
        )
        .with_policy(parts.mode, AllowList::parse(""))
        .with_cancel(parts.cancel)
        .with_approver(parts.approver))
    });
    let mut app = app(workspace.path());

    send(&mut app, &mut worker, "twice");

    // One key only: the second identical command must not ask again.
    pump_answering(&mut app, &worker, &[KeyCode::Char('a')]);

    assert_eq!(
        std::fs::read_to_string(workspace.path().join("twice.txt")).expect("file"),
        "once\n"
    );
    assert_eq!(app.status.turn, TurnState::Idle);
}

#[test]
fn refusing_a_gated_command_reports_it_to_the_model() {
    let workspace = tempfile::tempdir().expect("workspace");
    let turns = Arc::new(Mutex::new(VecDeque::from(vec![
        tool_turn("call-1", "shell", r#"{"command":"touch refused.txt"}"#),
        text_turn("understood"),
    ])));
    let build_turns = Arc::clone(&turns);
    let ctx = ToolContext::new(workspace.path()).expect("workspace");

    let mut worker = WorkerHandle::spawn("mock:mock-model", move |parts| {
        Ok(Agent::new(
            Box::new(Scripted {
                turns: Arc::clone(&build_turns),
            }),
            ctx.clone(),
            AgentConfig {
                model: "mock-model".to_owned(),
                ..AgentConfig::default()
            },
        )
        .with_policy(parts.mode, AllowList::parse(""))
        .with_cancel(parts.cancel)
        .with_approver(parts.approver))
    });
    let mut app = app(workspace.path());

    send(&mut app, &mut worker, "touch it");
    pump_answering(&mut app, &worker, &[KeyCode::Char('n')]);

    assert!(!workspace.path().join("refused.txt").exists());
    let failed = app
        .transcript
        .rows()
        .iter()
        .find(|row| row.kind == niki_tui_live::RowKind::Error)
        .expect("a refused result");
    assert!(failed.text.contains("not approved"), "{}", failed.text);
    assert_eq!(app.status.turn, TurnState::Idle);
}

#[test]
fn a_worker_turn_is_written_to_the_session_store() {
    let workspace = tempfile::tempdir().expect("workspace");
    let store_home = tempfile::tempdir().expect("store");
    let path = store_home.path().join("sessions.db");
    let mut app = app(workspace.path());
    app.attach_sessions(niki_session::SessionStore::open(&path).expect("session store"));
    let session = app.session_id().expect("a session row").to_owned();

    let turns = Arc::new(Mutex::new(VecDeque::from(vec![text_turn("saved answer")])));
    let build_turns = Arc::clone(&turns);
    let ctx = ToolContext::new(workspace.path()).expect("workspace");
    let mut worker = WorkerHandle::spawn("mock:mock-model", move |parts| {
        Ok(Agent::new(
            Box::new(Scripted {
                turns: Arc::clone(&build_turns),
            }),
            ctx.clone(),
            AgentConfig {
                model: "mock-model".to_owned(),
                ..AgentConfig::default()
            },
        )
        .with_policy(parts.mode, AllowList::parse(""))
        .with_cancel(parts.cancel))
    });

    send(&mut app, &mut worker, "remember this");
    pump(&mut app, &worker);

    let store = niki_session::SessionStore::open(&path).expect("reopen the store");
    let summaries = store.list().expect("list");
    assert_eq!(summaries.len(), 1, "one session for one launch");
    let summary = summaries
        .iter()
        .find(|summary| summary.id.as_str() == session)
        .expect("the screen's session");
    assert_eq!(summary.messages, 2, "the prompt and the answer");
    let records: Vec<(String, String)> = store
        .load(&summary.id)
        .expect("load")
        .into_iter()
        .map(|message| (message.role, message.content))
        .collect();
    assert_eq!(
        records,
        [
            ("user".to_owned(), "remember this".to_owned()),
            ("assistant".to_owned(), "saved answer".to_owned()),
        ]
    );
}
