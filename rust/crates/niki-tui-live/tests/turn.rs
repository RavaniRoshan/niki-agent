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
use niki_provider::{ChatRequest, Provider, ProviderError, Role, StopReason, StreamEvent, Usage};
use niki_tools::{AskAnswer, AskError, ToolContext};
use niki_tui_live::{
    App, Command, Modal, QuestionReply, RowKind, SessionInfo, TurnState, WorkerEvent, WorkerHandle,
};

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

/// A provider that records every request: the first waits for the test, later
/// ones answer at once. A steer is proved by the second request carrying it.
struct RecordingGate {
    gate: Arc<Mutex<Receiver<()>>>,
    seen: Arc<Mutex<Vec<ChatRequest>>>,
    calls: Arc<Mutex<usize>>,
}

impl Provider for RecordingGate {
    fn name(&self) -> &str {
        "recording"
    }

    fn stream(
        &self,
        request: ChatRequest,
    ) -> BoxStream<'static, Result<StreamEvent, ProviderError>> {
        self.seen.lock().expect("lock").push(request);
        let call = {
            let mut calls = self.calls.lock().expect("lock");
            *calls += 1;
            *calls
        };
        if call == 1 {
            Box::pin(Gated {
                prefix: Some(StreamEvent::TextDelta("half an answer".to_owned())),
                suffix: Some(StreamEvent::Done(StopReason::EndTurn)),
                gate: Arc::clone(&self.gate),
                state: 0,
            })
        } else {
            futures_util::stream::iter(text_turn("answer to the steer").into_iter().map(Ok)).boxed()
        }
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
    app_in(root, Mode::Manual)
}

/// The app with a chosen approval mode, for tests that let the worker act.
fn app_in(root: &std::path::Path, mode: Mode) -> App {
    App::new(SessionInfo {
        model: "mock:mock-model".to_owned(),
        mode,
        cwd: root.to_path_buf(),
    })
}

/// Type text into the composer without sending it.
fn type_into(app: &mut App, text: &str) {
    for character in text.chars() {
        let _ = app.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
    }
}

/// Type a prompt into the composer and send it, as the screen does.
fn send(app: &mut App, worker: &mut WorkerHandle, prompt: &str) {
    type_into(app, prompt);
    match app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)) {
        Some(Command::Submit(text)) => assert!(worker.submit(text)),
        other => panic!("expected a prompt, got {other:?}"),
    }
}

/// Read worker messages until the turn finishes, folding them into `app`, and
/// hand the worker the prompt `finish` says was queued.
fn pump(app: &mut App, worker: &mut WorkerHandle) -> Vec<AgentEvent> {
    let mut seen = Vec::new();
    loop {
        match worker.recv_timeout(Duration::from_secs(10)) {
            Some(WorkerEvent::Agent(event)) => {
                seen.push(event.clone());
                app.apply(event);
            }
            Some(WorkerEvent::Finished(result)) => {
                let next = app.finish(result.map_err(|message| message.to_string()));
                if let Some(Command::Submit(prompt)) = next {
                    assert!(worker.submit(prompt), "the worker is running");
                }
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
            Some(WorkerEvent::Audit(audit)) => app.show_audit(audit),
            Some(WorkerEvent::Compacted { result, fallback }) => app.compacted(result, fallback),
            Some(WorkerEvent::Approval { .. }) => panic!("this worker installs no approver"),
            Some(WorkerEvent::Question { .. }) => panic!("this worker installs no asker"),
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
    let events = pump(&mut app, &mut worker);

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
    pump(&mut app, &mut worker);
    send(&mut app, &mut worker, "second");
    pump(&mut app, &mut worker);

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
    pump(&mut app, &mut worker);
    send(&mut app, &mut worker, "second");
    pump(&mut app, &mut worker);

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
            WorkerEvent::Audit(_) => {}
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

    let mut worker = WorkerHandle::spawn_with("mock:mock-model", Mode::Auto, move |parts| {
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
    let mut app = app_in(workspace.path(), Mode::Auto);

    send(&mut app, &mut worker, "write note.txt");
    pump(&mut app, &mut worker);

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
                assert_eq!(app.finish(Ok(outcome)), None);
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
            WorkerEvent::Audit(audit) => app.show_audit(audit),
            WorkerEvent::Compacted { result, fallback } => app.compacted(result, fallback),
            WorkerEvent::Approval { .. } => panic!("this worker installs no approver"),
            WorkerEvent::Question { .. } => panic!("this worker installs no asker"),
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
            assert_eq!(app.finish(Err(message.clone())), None);
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
fn pump_answering(app: &mut App, worker: &mut WorkerHandle, keys: &[KeyCode]) -> Vec<AgentEvent> {
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
                let next = app.finish(result.map_err(|message| message.to_string()));
                if let Some(Command::Submit(prompt)) = next {
                    assert!(worker.submit(prompt), "the worker is running");
                }
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
            Some(WorkerEvent::Audit(audit)) => app.show_audit(audit),
            Some(WorkerEvent::Compacted { result, fallback }) => app.compacted(result, fallback),
            Some(WorkerEvent::Question { .. }) => panic!("no question in this test"),
            None => panic!("the worker went quiet before the turn finished"),
        }
    }
}

/// Drive keys on the screen and pass a structured question's answer back over
/// the reply channel, which is what the event loop does.
fn pump_question(app: &mut App, worker: &mut WorkerHandle, keys: &[KeyCode]) -> Vec<AgentEvent> {
    let mut seen = Vec::new();
    let mut pending_keys: VecDeque<KeyCode> = keys.iter().copied().collect();
    let mut asked = false;
    loop {
        match worker.recv_timeout(Duration::from_secs(10)) {
            Some(WorkerEvent::Agent(event)) => {
                seen.push(event.clone());
                app.apply(event);
            }
            Some(WorkerEvent::Question { request, reply }) => {
                asked = true;
                app.request_question(request);
                assert!(
                    matches!(app.modal, Some(Modal::Question(_))),
                    "the question prompt is open"
                );
                assert_eq!(app.status.turn, TurnState::Waiting);
                let mut answered = false;
                while !answered {
                    let key = pending_keys.pop_front().expect("a key for the question");
                    match app.handle_key(KeyEvent::new(key, KeyModifiers::NONE)) {
                        Some(Command::AnswerQuestion(answer)) => {
                            let result = match answer {
                                QuestionReply::Choice(label) => Ok(AskAnswer::Choice(label)),
                                QuestionReply::Freeform(text) => Ok(AskAnswer::Freeform(text)),
                                QuestionReply::Cancelled => Err(AskError::Failed(
                                    "the question was cancelled; ask again or continue without it"
                                        .to_owned(),
                                )),
                            };
                            reply.send(result).expect("answer");
                            answered = true;
                        }
                        Some(other) => panic!("unexpected command from the question: {other:?}"),
                        None => {}
                    }
                }
            }
            Some(WorkerEvent::Finished(result)) => {
                assert!(asked, "the model asked and the screen answered");
                let next = app.finish(result.map_err(|message| message.to_string()));
                if let Some(Command::Submit(prompt)) = next {
                    assert!(worker.submit(prompt), "the worker is running");
                }
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
            Some(WorkerEvent::Audit(audit)) => app.show_audit(audit),
            Some(WorkerEvent::Compacted { result, fallback }) => app.compacted(result, fallback),
            Some(WorkerEvent::Approval { .. }) => panic!("no approval in this test"),
            None => panic!("the worker went quiet before the turn finished"),
        }
    }
}

#[test]
fn a_question_answer_continues_the_turn_with_the_chosen_option() {
    let workspace = tempfile::tempdir().expect("workspace");
    let turns = Arc::new(Mutex::new(VecDeque::from(vec![
        tool_turn(
            "call-1",
            "request_user_input",
            r#"{"question":"Which store?","options":[{"label":"SQLite","description":"one file, no server"},{"label":"JSON"}]}"#,
        ),
        text_turn("storing in JSON"),
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

    send(&mut app, &mut worker, "ask me");
    let seen = pump_question(&mut app, &mut worker, &[KeyCode::Down, KeyCode::Enter]);

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
    assert_eq!(tool_end.0, "request_user_input");
    assert_eq!(tool_end.1, "JSON", "the second option is the answer");
    assert!(!tool_end.2);
    assert!(app.modal.is_none(), "the prompt closed");
    assert!(app.question.is_none());
    assert_eq!(app.status.turn, TurnState::Idle);
    assert!(
        app.transcript
            .rows()
            .iter()
            .any(|row| row.kind == RowKind::Assistant && row.text.contains("storing in JSON")),
        "the turn continued: {:?}",
        app.transcript.rows()
    );
}

#[test]
fn escaping_a_question_reports_a_dismissal_and_the_turn_continues() {
    let workspace = tempfile::tempdir().expect("workspace");
    let turns = Arc::new(Mutex::new(VecDeque::from(vec![
        tool_turn(
            "call-1",
            "request_user_input",
            r#"{"question":"Which store?","options":[{"label":"SQLite"},{"label":"JSON"}]}"#,
        ),
        text_turn("I will pick one and say why"),
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

    send(&mut app, &mut worker, "ask me");
    let seen = pump_question(&mut app, &mut worker, &[KeyCode::Esc]);

    let tool_end = seen
        .iter()
        .find_map(|event| match event {
            AgentEvent::ToolEnd {
                output, is_error, ..
            } => Some((output.as_str(), *is_error)),
            _ => None,
        })
        .expect("a tool result");
    assert!(tool_end.1, "the cancellation is a tool error");
    assert!(
        tool_end.0.contains("the question was cancelled"),
        "{}",
        tool_end.0
    );
    assert!(
        seen.iter().any(
            |event| matches!(event, AgentEvent::Text(text) if text == "I will pick one and say why")
        ),
        "the turn continued after the cancellation: {seen:?}"
    );
    assert_eq!(app.status.turn, TurnState::Idle);
    assert!(app.modal.is_none());
}

#[test]
fn a_free_text_question_returns_the_typed_answer_with_its_leading_q() {
    let workspace = tempfile::tempdir().expect("workspace");
    let turns = Arc::new(Mutex::new(VecDeque::from(vec![
        tool_turn(
            "call-1",
            "request_user_input",
            r#"{"question":"Which store?","options":[{"label":"SQLite"},{"label":"JSON"}],"allow_freeform":true}"#,
        ),
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
        .with_cancel(parts.cancel))
    });
    let mut app = app(workspace.path());

    send(&mut app, &mut worker, "ask me");
    let seen = pump_question(
        &mut app,
        &mut worker,
        &[
            KeyCode::Char('q'),
            KeyCode::Char(' '),
            KeyCode::Char('W'),
            KeyCode::Char('A'),
            KeyCode::Char('L'),
            KeyCode::Enter,
        ],
    );

    let tool_end = seen
        .iter()
        .find_map(|event| match event {
            AgentEvent::ToolEnd {
                output, is_error, ..
            } => Some((output.as_str(), *is_error)),
            _ => None,
        })
        .expect("a tool result");
    assert_eq!(tool_end.0, "q WAL", "q is text, not a close key");
    assert!(!tool_end.1);
    assert_eq!(app.status.turn, TurnState::Idle);
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

    let seen = pump_answering(&mut app, &mut worker, &[KeyCode::Char('y')]);

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
fn a_session_answer_covers_the_next_command_of_the_same_program() {
    let workspace = tempfile::tempdir().expect("workspace");
    let turns = Arc::new(Mutex::new(VecDeque::from(vec![
        tool_turn("call-1", "shell", r#"{"command":"echo once"}"#),
        tool_turn("call-2", "shell", r#"{"command":"echo twice"}"#),
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

    // One key only: the second command of the same program must not ask again.
    pump_answering(&mut app, &mut worker, &[KeyCode::Char('s')]);

    assert_eq!(app.status.turn, TurnState::Idle);
}

#[test]
fn a_session_answer_does_not_cover_a_new_redirect() {
    let workspace = tempfile::tempdir().expect("workspace");
    let turns = Arc::new(Mutex::new(VecDeque::from(vec![
        tool_turn("call-1", "shell", r#"{"command":"echo once > twice.txt"}"#),
        tool_turn("call-2", "shell", r#"{"command":"echo twice > twice.txt"}"#),
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

    // Two keys: a second redirect of the same program is a new question.
    pump_answering(
        &mut app,
        &mut worker,
        &[KeyCode::Char('s'), KeyCode::Char('s')],
    );

    assert_eq!(
        std::fs::read_to_string(workspace.path().join("twice.txt")).expect("file"),
        "twice\n"
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
    pump_answering(&mut app, &mut worker, &[KeyCode::Char('n')]);

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
    pump(&mut app, &mut worker);

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

/// Type a line on the screen, press Enter, and hand a mode command to the
/// worker the way the event loop does.
fn send_mode(app: &mut App, worker: &mut WorkerHandle, line: &str) -> Mode {
    for character in line.chars() {
        let _ = app.handle_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
    }
    match app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)) {
        Some(Command::Mode(mode)) => {
            assert!(worker.set_mode(mode), "the worker is running");
            mode
        }
        other => panic!("expected a mode, got {other:?}"),
    }
}

#[test]
fn every_mode_command_sets_the_mode_the_next_turn_is_built_with() {
    for (command, expected) in [
        ("/manual", Mode::Manual),
        ("/auto", Mode::Auto),
        ("/yolo", Mode::Yolo),
    ] {
        let workspace = tempfile::tempdir().expect("workspace");
        let turns = Arc::new(Mutex::new(VecDeque::from(vec![text_turn("ok")])));
        let build_turns = Arc::clone(&turns);
        let ctx = ToolContext::new(workspace.path()).expect("workspace");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&seen);

        let mut worker = WorkerHandle::spawn_with("mock:mock-model", Mode::Manual, move |parts| {
            record.lock().expect("record").push(parts.mode);
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

        assert_eq!(
            send_mode(&mut app, &mut worker, command),
            expected,
            "{command}"
        );
        assert_eq!(app.status.mode, expected, "{command}");
        assert!(
            app.status_line().contains(expected.label()),
            "{command}: {}",
            app.status_line()
        );

        send(&mut app, &mut worker, "next turn");
        pump(&mut app, &mut worker);
        assert_eq!(
            *seen.lock().expect("record"),
            [expected],
            "{command} must reach SessionParts::mode"
        );
    }
}

#[test]
fn the_worker_keeps_one_mode_across_rebuilds() {
    let workspace = tempfile::tempdir().expect("workspace");
    let turns = Arc::new(Mutex::new(VecDeque::from(vec![
        text_turn("one"),
        text_turn("two"),
        text_turn("three"),
    ])));
    let build_turns = Arc::clone(&turns);
    let ctx = ToolContext::new(workspace.path()).expect("workspace");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&seen);

    let mut worker = WorkerHandle::spawn_with("mock:mock-model", Mode::Manual, move |parts| {
        record.lock().expect("record").push(parts.mode);
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
    pump(&mut app, &mut worker);

    for (command, expected) in [("/yolo", Mode::Yolo), ("/auto", Mode::Auto)] {
        assert_eq!(send_mode(&mut app, &mut worker, command), expected);
        assert!(worker.select_model("mock:mock-model"), "force a rebuild");
        send(&mut app, &mut worker, "again");
        pump(&mut app, &mut worker);
    }

    assert_eq!(
        *seen.lock().expect("record"),
        [Mode::Manual, Mode::Yolo, Mode::Auto],
        "every rebuild takes the mode the worker holds, not the one it started with"
    );
}

#[test]
fn a_queued_prompt_is_submitted_when_the_turn_ends_and_only_then() {
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

    send(&mut app, &mut worker, "first");
    match worker
        .recv_timeout(Duration::from_secs(10))
        .expect("the first delta")
    {
        WorkerEvent::Agent(event @ AgentEvent::Text(_)) => app.apply(event),
        other => panic!("expected answer text, got {other:?}"),
    }

    type_into(&mut app, "second");
    assert_eq!(
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        None,
        "the turn is still running, so the prompt is queued"
    );
    assert_eq!(app.queued().len(), 1);
    assert!(
        worker.try_recv().is_none(),
        "nothing was sent to the worker"
    );

    release.send(()).expect("release the first turn");
    pump(&mut app, &mut worker);
    assert_eq!(
        app.status.turn,
        TurnState::Working,
        "the queued prompt started when the turn ended"
    );
    assert!(app.queued().is_empty());

    release.send(()).expect("release the second turn");
    pump(&mut app, &mut worker);

    let users: Vec<&str> = app
        .transcript
        .rows()
        .iter()
        .filter(|row| row.kind == RowKind::User)
        .map(|row| row.text.as_str())
        .collect();
    assert_eq!(users, ["first", "second"]);
    assert_eq!(app.status.turn, TurnState::Idle);
}

#[test]
fn steering_joins_the_turn_that_is_running_without_starting_another() {
    let workspace = tempfile::tempdir().expect("workspace");
    let (release, gate) = mpsc::channel();
    let gate = Arc::new(Mutex::new(gate));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let calls = Arc::new(Mutex::new(0usize));
    let (build_gate, build_seen, build_calls) =
        (Arc::clone(&gate), Arc::clone(&seen), Arc::clone(&calls));
    let ctx = ToolContext::new(workspace.path()).expect("workspace");

    let mut worker = WorkerHandle::spawn("mock:mock-model", move |parts| {
        Ok(Agent::new(
            Box::new(RecordingGate {
                gate: Arc::clone(&build_gate),
                seen: Arc::clone(&build_seen),
                calls: Arc::clone(&build_calls),
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
    match worker
        .recv_timeout(Duration::from_secs(10))
        .expect("the first delta")
    {
        WorkerEvent::Agent(event @ AgentEvent::Text(_)) => app.apply(event),
        other => panic!("expected answer text, got {other:?}"),
    }
    assert_eq!(app.status.turn, TurnState::Working);

    type_into(&mut app, "look at the tests");
    match app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT)) {
        Some(Command::Steer(text)) => worker.steer(text),
        other => panic!("expected a steer, got {other:?}"),
    }
    assert!(app.queued().is_empty(), "a steer is not a queued prompt");

    release.send(()).expect("release the stream");
    pump(&mut app, &mut worker);

    let requests = seen.lock().expect("lock").clone();
    assert_eq!(requests.len(), 2, "one turn asked twice, not two turns");
    assert!(
        requests[1]
            .messages
            .iter()
            .any(|message| message.role == Role::User && message.content == "look at the tests"),
        "{:?}",
        requests[1].messages
    );
    assert_eq!(app.status.turn, TurnState::Idle);
    assert!(
        worker.try_recv().is_none(),
        "no second turn was started behind it"
    );
    let users: Vec<&str> = app
        .transcript
        .rows()
        .iter()
        .filter(|row| row.kind == RowKind::User)
        .map(|row| row.text.as_str())
        .collect();
    assert_eq!(users, ["first", "look at the tests"]);
}

#[test]
fn a_bang_line_runs_locally_and_the_model_is_never_asked() {
    let workspace = tempfile::tempdir().expect("workspace");
    let turns = Arc::new(Mutex::new(VecDeque::from(vec![text_turn(
        "the model was asked",
    )])));
    let build_turns = Arc::clone(&turns);
    let ctx = ToolContext::new(workspace.path()).expect("workspace");

    let worker = WorkerHandle::spawn("mock:mock-model", move |parts| {
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
    app.status.mode = Mode::Yolo;

    type_into(&mut app, "!printf hi > out.txt; printf oops >&2; exit 3");
    match app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)) {
        Some(Command::Shell(command)) => assert!(worker.shell(command, workspace.path())),
        other => panic!("expected the shell line, got {other:?}"),
    }

    let event = match worker
        .recv_timeout(Duration::from_secs(10))
        .expect("the shell result")
    {
        WorkerEvent::Agent(event @ AgentEvent::ToolEnd { .. }) => {
            app.apply(event.clone());
            event
        }
        other => panic!("the model must not be involved, but got {other:?}"),
    };
    let AgentEvent::ToolEnd {
        name,
        output,
        is_error,
    } = event
    else {
        unreachable!("the loop only breaks on a tool end")
    };
    assert_eq!(name, "shell");
    assert!(!is_error, "{output}");
    assert!(output.contains("exit_code: 3"), "{output}");
    assert!(output.contains("oops"), "stderr is in the row: {output}");
    assert_eq!(
        std::fs::read_to_string(workspace.path().join("out.txt")).expect("file"),
        "hi"
    );

    let rows = app.transcript.rows();
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0].kind, RowKind::ToolCall);
    assert_eq!(rows[0].title.as_deref(), Some("shell"));
    assert!(rows[0].text.contains("printf hi"), "{}", rows[0].text);
    assert_eq!(rows[1].kind, RowKind::ToolResult);
    assert!(rows[1].text.contains("exit_code: 3"), "{}", rows[1].text);

    assert_eq!(
        turns.lock().expect("lock").len(),
        1,
        "the model was never asked"
    );
    assert_eq!(app.status.tokens, Usage::default(), "no tokens were spent");
    assert_eq!(app.status.turn, TurnState::Idle);
    assert!(worker.try_recv().is_none(), "the worker added nothing else");
}
