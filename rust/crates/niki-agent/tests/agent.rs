use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use niki_agent::{Agent, AgentConfig, AgentEvent, AllowList, CancelHandle, Mode};
use niki_provider::{ChatRequest, Provider, ProviderError, Role, StopReason, StreamEvent, Usage};
use niki_tools::ToolContext;

/// A provider that replays one scripted turn per request and records what it
/// was asked. No network, so the loop is testable anywhere (RULES.md 8).
#[derive(Clone)]
struct Scripted {
    turns: Arc<Mutex<VecDeque<Vec<StreamEvent>>>>,
    seen: Arc<Mutex<Vec<ChatRequest>>>,
}

impl Scripted {
    fn new(turns: Vec<Vec<StreamEvent>>) -> Self {
        Self {
            turns: Arc::new(Mutex::new(turns.into())),
            seen: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn requests(&self) -> Vec<ChatRequest> {
        self.seen.lock().expect("lock").clone()
    }
}

impl Provider for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }

    fn stream(
        &self,
        request: ChatRequest,
    ) -> BoxStream<'static, Result<StreamEvent, ProviderError>> {
        self.seen.lock().expect("lock").push(request);
        let events = self
            .turns
            .lock()
            .expect("lock")
            .pop_front()
            .unwrap_or_default();
        futures_util::stream::iter(events.into_iter().map(Ok)).boxed()
    }
}

fn text_turn(text: &str) -> Vec<StreamEvent> {
    vec![
        StreamEvent::TextDelta(text.to_owned()),
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
        StreamEvent::Usage(Usage {
            input_tokens: 10,
            output_tokens: 4,
        }),
        StreamEvent::Done(StopReason::ToolUse),
    ]
}

fn agent(provider: Scripted, root: &std::path::Path, config: AgentConfig) -> Agent {
    Agent::new(
        Box::new(provider),
        ToolContext::new(root).expect("workspace"),
        config,
    )
}

fn config() -> AgentConfig {
    AgentConfig {
        model: "test-model".to_owned(),
        ..AgentConfig::default()
    }
}

#[tokio::test]
async fn one_text_turn_keeps_the_transcript() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![text_turn("hello")]);
    let mut agent = agent(provider, dir.path(), config());

    let mut events = Vec::new();
    let outcome = agent
        .run("hi", &mut |event| events.push(event.clone()))
        .await
        .expect("run");

    assert_eq!(outcome.text, "hello");
    assert_eq!(outcome.steps, 1);
    assert_eq!(outcome.tool_calls, 0);
    assert_eq!(outcome.stop, StopReason::EndTurn);
    assert!(events.contains(&AgentEvent::Text("hello".to_owned())));

    let messages = agent.messages();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, Role::User);
    assert_eq!(messages[0].content, "hi");
    assert_eq!(messages[1].role, Role::Assistant);
    assert_eq!(messages[1].content, "hello");
}

#[tokio::test]
async fn a_tool_call_runs_and_feeds_the_result_back() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        tool_turn(
            "call-1",
            "write_file",
            r#"{"path":"note.txt","content":"made by niki"}"#,
        ),
        text_turn("done"),
    ]);
    let observed = provider.clone();
    let mut agent = agent(provider, dir.path(), config());

    let outcome = agent.run("write it", &mut |_| {}).await.expect("run");

    assert_eq!(outcome.text, "done");
    assert_eq!(outcome.steps, 2);
    assert_eq!(outcome.tool_calls, 1);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("note.txt")).expect("file"),
        "made by niki"
    );

    let messages = agent.messages();
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[2].role, Role::Tool);
    assert_eq!(messages[2].tool_call_id.as_deref(), Some("call-1"));
    assert!(messages[2].content.contains("note.txt"));

    let requests = observed.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].model, "test-model");
    assert!(
        requests[0]
            .tools
            .iter()
            .any(|tool| tool.name == "write_file")
    );
    assert_eq!(requests[1].messages.len(), 3);
}

#[tokio::test]
async fn manual_mode_refuses_a_shell_command_nobody_approved() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        tool_turn("call-1", "shell", r#"{"command":"touch escaped.txt"}"#),
        text_turn("understood"),
    ]);
    let mut agent =
        agent(provider, dir.path(), config()).with_policy(Mode::Manual, AllowList::parse(""));

    let mut events = Vec::new();
    let outcome = agent
        .run("touch it", &mut |event| events.push(event.clone()))
        .await
        .expect("run");

    assert!(!dir.path().join("escaped.txt").exists());
    assert_eq!(outcome.tool_calls, 1);
    let refusal = agent.messages()[2].content.clone();
    assert!(refusal.contains("refused"), "{refusal}");
    assert!(refusal.contains("approval"), "{refusal}");
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::ToolEnd { is_error: true, .. }))
    );
}

#[tokio::test]
async fn an_allow_listed_command_runs_in_manual_mode() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        tool_turn("call-1", "shell", r#"{"command":"echo hi > ran.txt"}"#),
        text_turn("done"),
    ]);
    let mut agent =
        agent(provider, dir.path(), config()).with_policy(Mode::Manual, AllowList::parse("echo*"));

    let outcome = agent.run("run it", &mut |_| {}).await.expect("run");

    assert_eq!(outcome.tool_calls, 1);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("ran.txt")).expect("file"),
        "hi\n"
    );
}

#[tokio::test]
async fn the_step_budget_stops_a_model_that_never_finishes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        tool_turn("c1", "list_dir", r#"{"path":"."}"#),
        tool_turn("c2", "list_dir", r#"{"path":"."}"#),
        tool_turn("c3", "list_dir", r#"{"path":"."}"#),
        text_turn("never reached"),
    ]);
    let mut agent = agent(
        provider,
        dir.path(),
        AgentConfig {
            max_steps: 2,
            ..config()
        },
    );

    let mut events = Vec::new();
    let outcome = agent
        .run("loop", &mut |event| events.push(event.clone()))
        .await
        .expect("run");

    assert_eq!(outcome.steps, 2);
    assert_eq!(outcome.tool_calls, 2);
    assert_eq!(
        outcome.stop,
        StopReason::Other("step limit 2 reached".to_owned())
    );
    assert!(events.contains(&AgentEvent::StepLimit { steps: 2 }));
}

#[tokio::test]
async fn a_cancelled_turn_stops_without_losing_the_transcript() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cancel = CancelHandle::new();
    cancel.cancel();
    let provider = Scripted::new(vec![text_turn("never reached")]);
    let mut agent = agent(provider, dir.path(), config()).with_cancel(cancel);

    let outcome = agent.run("hi", &mut |_| {}).await.expect("run");

    assert_eq!(outcome.steps, 0);
    assert_eq!(outcome.stop, StopReason::Other("cancelled".to_owned()));
    assert_eq!(agent.messages().len(), 1);
}

#[tokio::test]
async fn an_unknown_tool_becomes_a_result_the_model_can_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        tool_turn("call-1", "teleport", r#"{}"#),
        text_turn("sorry"),
    ]);
    let mut agent = agent(provider, dir.path(), config());

    let outcome = agent.run("go", &mut |_| {}).await.expect("run");

    assert_eq!(outcome.tool_calls, 1);
    let message = &agent.messages()[2];
    assert!(
        message.content.contains("no tool named"),
        "{}",
        message.content
    );
}
