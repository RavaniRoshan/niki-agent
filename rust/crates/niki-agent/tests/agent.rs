use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use niki_agent::{Agent, AgentConfig, AgentEvent, AllowList, ApprovalAction, CancelHandle, Mode};
use niki_hooks::Notify;
use niki_provider::{ChatRequest, Provider, ProviderError, Role, StopReason, StreamEvent, Usage};
use niki_tools::{Registry, Tool, ToolContext, ToolError, ToolOutput};
use serde_json::{Value, json};

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
    let mut agent =
        agent(provider, dir.path(), config()).with_policy(Mode::Auto, AllowList::parse(""));

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
        tool_turn("call-1", "shell", r#"{"command":"touch ran.txt"}"#),
        text_turn("done"),
    ]);
    let mut agent =
        agent(provider, dir.path(), config()).with_policy(Mode::Manual, AllowList::parse("touch*"));

    let outcome = agent.run("run it", &mut |_| {}).await.expect("run");

    assert_eq!(outcome.tool_calls, 1);
    assert!(dir.path().join("ran.txt").exists());
}

#[tokio::test]
async fn a_chained_command_is_not_covered_by_a_prefix_entry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        tool_turn(
            "call-1",
            "shell",
            r#"{"command":"echo one; touch injected.txt"}"#,
        ),
        text_turn("understood"),
    ]);
    let mut agent =
        agent(provider, dir.path(), config()).with_policy(Mode::Manual, AllowList::parse("echo*"));

    let outcome = agent.run("run it", &mut |_| {}).await.expect("run");

    assert_eq!(outcome.tool_calls, 1);
    assert!(
        !dir.path().join("injected.txt").exists(),
        "the chained command must not run without approval"
    );
    let refusal = agent.messages()[2].content.clone();
    assert!(refusal.contains("refused"), "{refusal}");
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

/// An approver that records every request and answers with `answer`.
fn approver(
    answer: niki_agent::Approval,
) -> (
    niki_agent::Approver,
    Arc<Mutex<Vec<niki_agent::ApprovalRequest>>>,
) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&seen);
    let approver: niki_agent::Approver = Box::new(move |request| {
        recorded.lock().expect("lock").push(request);
        Box::pin(async move { answer })
    });
    (approver, seen)
}

#[tokio::test]
async fn an_approver_runs_a_gated_command_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        tool_turn("call-1", "shell", r#"{"command":"echo hi > asked.txt"}"#),
        text_turn("done"),
    ]);
    let (approver, seen) = approver(niki_agent::Approval::Once);
    let mut agent = agent(provider, dir.path(), config()).with_approver(approver);

    let outcome = agent.run("run it", &mut |_| {}).await.expect("run");

    assert_eq!(outcome.tool_calls, 1);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("asked.txt")).expect("file"),
        "hi\n"
    );
    let requests = seen.lock().expect("lock");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].command, "echo hi > asked.txt");
    assert_eq!(requests[0].mode, Mode::Manual);
}

#[tokio::test]
async fn denying_a_gated_command_is_a_refusal_the_model_reads() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        tool_turn("call-1", "shell", r#"{"command":"touch denied.txt"}"#),
        text_turn("understood"),
    ]);
    let (approver, seen) = approver(niki_agent::Approval::Deny);
    let mut agent = agent(provider, dir.path(), config()).with_approver(approver);

    let outcome = agent.run("touch it", &mut |_| {}).await.expect("run");

    assert!(!dir.path().join("denied.txt").exists());
    assert_eq!(seen.lock().expect("lock").len(), 1);
    assert_eq!(outcome.text, "understood");
    assert!(
        agent.messages()[2].content.contains("not approved"),
        "{}",
        agent.messages()[2].content
    );
}

/// An external tool that records whether it ran, so a refusal can be told
/// apart from an execution.
struct ExternalWrite {
    name: &'static str,
    gated: bool,
    runs: Arc<std::sync::atomic::AtomicUsize>,
}

impl Tool for ExternalWrite {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        "An external tool for the approval tests."
    }

    fn schema(&self) -> Value {
        json!({"type": "object"})
    }

    fn requires_approval(&self) -> bool {
        self.gated
    }

    async fn run(&self, _args: Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        self.runs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(ToolOutput::new("changed"))
    }
}

fn external_registry(
    name: &'static str,
    gated: bool,
) -> (Registry, Arc<std::sync::atomic::AtomicUsize>) {
    let runs = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut registry = Registry::new();
    registry.register(ExternalWrite {
        name,
        gated,
        runs: Arc::clone(&runs),
    });
    (registry, runs)
}

#[tokio::test]
async fn a_mutating_external_tool_asks_before_it_runs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (registry, runs) = external_registry("mcp__fixture__write", true);
    let provider = Scripted::new(vec![
        tool_turn("call-1", "mcp__fixture__write", r#"{"path":"note.txt"}"#),
        text_turn("done"),
    ]);
    let (approver, seen) = approver(niki_agent::Approval::Once);
    let mut agent = agent(provider, dir.path(), config())
        .with_registry(registry)
        .with_approver(approver);

    let outcome = agent.run("write it", &mut |_| {}).await.expect("run");

    assert_eq!(outcome.tool_calls, 1);
    assert_eq!(runs.load(std::sync::atomic::Ordering::SeqCst), 1);
    let requests = seen.lock().expect("lock");
    assert_eq!(requests.len(), 1);
    assert!(requests[0].command.starts_with("mcp__fixture__write"));
}

#[tokio::test]
async fn a_mutating_external_tool_without_an_approver_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (registry, runs) = external_registry("mcp__fixture__write", true);
    let provider = Scripted::new(vec![
        tool_turn("call-1", "mcp__fixture__write", "{}"),
        text_turn("understood"),
    ]);
    let mut agent = agent(provider, dir.path(), config()).with_registry(registry);

    let outcome = agent.run("write it", &mut |_| {}).await.expect("run");

    assert_eq!(outcome.tool_calls, 1);
    assert_eq!(runs.load(std::sync::atomic::Ordering::SeqCst), 0);
    let refusal = &agent.messages()[2].content;
    assert!(refusal.contains("refused"), "{refusal}");
    assert!(refusal.contains("mcp__fixture__write"), "{refusal}");
}

#[tokio::test]
async fn a_read_only_external_tool_runs_without_asking() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (registry, runs) = external_registry("mcp__fixture__peek", false);
    let provider = Scripted::new(vec![
        tool_turn("call-1", "mcp__fixture__peek", "{}"),
        text_turn("done"),
    ]);
    let (approver, seen) = approver(niki_agent::Approval::Deny);
    let mut agent = agent(provider, dir.path(), config())
        .with_registry(registry)
        .with_approver(approver);

    let outcome = agent.run("look", &mut |_| {}).await.expect("run");

    assert_eq!(outcome.tool_calls, 1);
    assert_eq!(runs.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(seen.lock().expect("lock").is_empty());
}

#[tokio::test]
async fn always_permits_the_same_command_without_asking_again() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        tool_turn("call-1", "shell", r#"{"command":"echo one > a.txt"}"#),
        tool_turn("call-2", "shell", r#"{"command":"echo one > a.txt"}"#),
        text_turn("done"),
    ]);
    let (approver, seen) = approver(niki_agent::Approval::Always);
    let mut agent = agent(provider, dir.path(), config()).with_approver(approver);

    let outcome = agent.run("twice", &mut |_| {}).await.expect("run");

    assert_eq!(outcome.tool_calls, 2);
    assert_eq!(
        seen.lock().expect("lock").len(),
        1,
        "the second identical command must not ask again"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).expect("file"),
        "one\n"
    );
}

#[tokio::test]
async fn a_session_answer_covers_the_same_program_without_asking_again() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        tool_turn("call-1", "shell", r#"{"command":"echo one > a.txt"}"#),
        tool_turn("call-2", "shell", r#"{"command":"echo two > b.txt"}"#),
        tool_turn("call-3", "shell", r#"{"command":"touch c.txt"}"#),
        text_turn("done"),
    ]);
    let (approver, seen) = approver(niki_agent::Approval::Always);
    let mut agent = agent(provider, dir.path(), config()).with_approver(approver);

    let outcome = agent.run("three commands", &mut |_| {}).await.expect("run");

    assert_eq!(outcome.tool_calls, 3);
    assert_eq!(
        seen.lock().expect("lock").len(),
        2,
        "echo covers its second command; touch is a different program"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).expect("file"),
        "one\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("b.txt")).expect("file"),
        "two\n"
    );
}

#[tokio::test]
async fn clearing_session_approvals_makes_the_program_ask_again() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        tool_turn("call-1", "shell", r#"{"command":"echo one > a.txt"}"#),
        text_turn("first"),
        tool_turn("call-2", "shell", r#"{"command":"echo two > b.txt"}"#),
        text_turn("second"),
    ]);
    let (approver, seen) = approver(niki_agent::Approval::Always);
    let mut agent = agent(provider, dir.path(), config()).with_approver(approver);

    agent.run("first", &mut |_| {}).await.expect("first run");
    assert_eq!(seen.lock().expect("lock").len(), 1);

    agent.clear_session_approvals();
    agent.run("second", &mut |_| {}).await.expect("second run");
    assert_eq!(
        seen.lock().expect("lock").len(),
        2,
        "a new session must not inherit the allow-for-session rule"
    );
}

#[tokio::test]
async fn a_write_question_names_the_absolute_path_and_the_boundary() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        tool_turn(
            "call-1",
            "write_file",
            r#"{"file_path":"../escape.txt","content":"nope"}"#,
        ),
        text_turn("understood"),
    ]);
    let (approver, seen) = approver(niki_agent::Approval::Once);
    let mut agent = agent(provider, dir.path(), config()).with_approver(approver);

    agent.run("write it", &mut |_| {}).await.expect("run");

    let requests = seen.lock().expect("lock");
    assert_eq!(requests.len(), 1);
    match &requests[0].action {
        ApprovalAction::Write { tool, path, inside } => {
            assert_eq!(tool, "write_file");
            assert!(path.is_absolute(), "{}", path.display());
            assert!(path.ends_with("escape.txt"), "{}", path.display());
            assert!(!inside, "the parent path escapes the workspace");
        }
        other => panic!("expected a write question, got {other:?}"),
    }
}

/// A workspace with a `hooks.json` the real runner reads, so the test exercises
/// the same parser and child process the binary uses.
fn hooks_document(dir: &std::path::Path, document: &str) -> niki_hooks::Hooks {
    let project = dir.join(".niki");
    std::fs::create_dir_all(&project).expect("hooks dir");
    std::fs::write(project.join("hooks.json"), document).expect("hooks file");
    let loaded = niki_hooks::Hooks::load_from(
        &project.join("hooks.json"),
        &dir.join("home").join("hooks.json"),
    );
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    loaded.hooks
}

#[tokio::test]
async fn a_blocking_hook_stops_a_tool_and_the_model_sees_the_reason() {
    let dir = tempfile::tempdir().expect("tempdir");
    let hooks = hooks_document(
        dir.path(),
        r#"{"hooks": {"PreToolUse": [{"matcher": "Write", "hooks": [
            {"type": "command", "command": "printf 'no writes today' >&2; exit 2"}
        ]}]}}"#,
    );
    let provider = Scripted::new(vec![
        tool_turn(
            "call-1",
            "write_file",
            r#"{"path":"blocked.txt","content":"nope"}"#,
        ),
        text_turn("understood"),
    ]);
    let observed = provider.clone();
    let mut agent = agent(provider, dir.path(), config()).with_hooks(std::sync::Arc::new(hooks));

    let mut events = Vec::new();
    let outcome = agent
        .run("write it", &mut |event| events.push(event.clone()))
        .await
        .expect("run");

    assert!(!dir.path().join("blocked.txt").exists());
    assert_eq!(outcome.tool_calls, 1);
    assert_eq!(outcome.text, "understood");

    let result = &agent.messages()[2].content;
    assert!(result.contains("no writes today"), "{result}");
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::ToolEnd { is_error: true, .. }))
    );

    let requests = observed.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].messages.len(), 3);
    assert!(
        requests[1].messages[2].content.contains("no writes today"),
        "{}",
        requests[1].messages[2].content
    );
}

#[tokio::test]
async fn a_blocking_prompt_hook_stops_the_turn_before_the_model() {
    let dir = tempfile::tempdir().expect("tempdir");
    let hooks = hooks_document(
        dir.path(),
        r#"{"hooks": {"UserPromptSubmit": [{"hooks": [
            {"type": "command", "command": "printf 'wait your turn' >&2; exit 2"}
        ]}]}}"#,
    );
    let provider = Scripted::new(vec![text_turn("never asked")]);
    let observed = provider.clone();
    let mut agent = agent(provider, dir.path(), config()).with_hooks(std::sync::Arc::new(hooks));

    let error = agent
        .run("hello", &mut |_| {})
        .await
        .expect_err("the prompt is stopped");

    assert!(matches!(
        &error,
        niki_agent::AgentError::HookBlocked { reason } if reason.as_str() == "wait your turn"
    ));
    assert!(observed.requests().is_empty());
    assert_eq!(agent.messages().len(), 1);
    assert_eq!(agent.messages()[0].content, "hello");
}

#[tokio::test]
async fn writing_a_file_asks_in_manual_mode() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        tool_turn(
            "call-1",
            "write_file",
            r#"{"file_path":"asked.txt","content":"hello"}"#,
        ),
        text_turn("understood"),
    ]);
    let (approver, seen) = approver(niki_agent::Approval::Deny);
    let mut agent = agent(provider, dir.path(), config())
        .with_policy(Mode::Manual, AllowList::parse(""))
        .with_approver(approver);

    agent.run("write it", &mut |_| {}).await.expect("run");

    let requests = seen.lock().expect("lock");
    assert_eq!(requests.len(), 1, "a write asks before it runs");
    match &requests[0].action {
        ApprovalAction::Write { tool, path, inside } => {
            assert_eq!(tool, "write_file");
            assert!(*inside, "asked.txt is inside the workspace");
            assert!(path.ends_with("asked.txt"), "{}", path.display());
        }
        other => panic!("expected a write question, got {other:?}"),
    }
    assert!(
        !dir.path().join("asked.txt").exists(),
        "a denied write does not touch the disk"
    );
}

#[tokio::test]
async fn auto_mode_writes_inside_the_workspace_without_asking() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        tool_turn(
            "call-1",
            "write_file",
            r#"{"file_path":"auto.txt","content":"hello"}"#,
        ),
        text_turn("done"),
    ]);
    let (approver, seen) = approver(niki_agent::Approval::Deny);
    let mut agent = agent(provider, dir.path(), config())
        .with_policy(Mode::Auto, AllowList::parse(""))
        .with_approver(approver);

    agent.run("write it", &mut |_| {}).await.expect("run");

    assert!(seen.lock().expect("lock").is_empty(), "Auto does not ask");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("auto.txt")).expect("written"),
        "hello"
    );
}

#[tokio::test]
async fn auto_mode_still_asks_for_a_path_outside_the_workspace() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        tool_turn(
            "call-1",
            "write_file",
            r#"{"file_path":"../escape.txt","content":"hello"}"#,
        ),
        text_turn("understood"),
    ]);
    let (approver, seen) = approver(niki_agent::Approval::Deny);
    let mut agent = agent(provider, dir.path(), config())
        .with_policy(Mode::Auto, AllowList::parse(""))
        .with_approver(approver);

    agent.run("write it", &mut |_| {}).await.expect("run");

    assert_eq!(
        seen.lock().expect("lock").len(),
        1,
        "a path that leaves the workspace asks"
    );
    assert!(
        !dir.path()
            .parent()
            .expect("parent")
            .join("escape.txt")
            .exists()
    );
}

#[tokio::test]
async fn auto_mode_reads_without_asking() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("seen.txt"), "text").expect("fixture");
    let provider = Scripted::new(vec![
        tool_turn("call-1", "read_file", r#"{"file_path":"seen.txt"}"#),
        text_turn("done"),
    ]);
    let (approver, seen) = approver(niki_agent::Approval::Deny);
    let mut agent = agent(provider, dir.path(), config())
        .with_policy(Mode::Auto, AllowList::parse(""))
        .with_approver(approver);

    agent.run("read it", &mut |_| {}).await.expect("run");

    assert!(seen.lock().expect("lock").is_empty(), "reads never ask");
}

/// A provider that steers the running turn while its first answer streams.
struct SteeringProvider {
    steering: niki_agent::Steering,
    turns: Arc<Mutex<VecDeque<Vec<StreamEvent>>>>,
    seen: Arc<Mutex<Vec<ChatRequest>>>,
}

impl Provider for SteeringProvider {
    fn name(&self) -> &str {
        "steering"
    }

    fn stream(
        &self,
        request: ChatRequest,
    ) -> BoxStream<'static, Result<StreamEvent, ProviderError>> {
        let first = self.seen.lock().expect("lock").is_empty();
        self.seen.lock().expect("lock").push(request);
        if first {
            self.steering.push("also mention the tests");
        }
        let events = self
            .turns
            .lock()
            .expect("lock")
            .pop_front()
            .unwrap_or_default();
        futures_util::stream::iter(events.into_iter().map(Ok)).boxed()
    }
}

#[tokio::test]
async fn a_steer_joins_the_running_turn_without_a_second_run() {
    let dir = tempfile::tempdir().expect("tempdir");
    let steering = niki_agent::Steering::new();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let provider = SteeringProvider {
        steering: steering.clone(),
        turns: Arc::new(Mutex::new(VecDeque::from(vec![
            text_turn("first answer"),
            text_turn("second answer"),
        ]))),
        seen: Arc::clone(&seen),
    };
    let mut agent = Agent::new(
        Box::new(provider),
        ToolContext::new(dir.path()).expect("workspace"),
        config(),
    )
    .with_steering(steering);

    let mut events = Vec::new();
    let outcome = agent
        .run("start", &mut |event| events.push(event.clone()))
        .await
        .expect("run");

    let requests = seen.lock().expect("lock").clone();
    assert_eq!(requests.len(), 2, "one run asked twice");
    assert!(
        requests[1].messages.iter().any(
            |message| message.role == Role::User && message.content == "also mention the tests"
        ),
        "{:?}",
        requests[1].messages
    );
    assert!(
        events.contains(&AgentEvent::Text("second answer".to_owned())),
        "the model answered the steer inside the same run"
    );
    assert_eq!(outcome.steps, 2);
    assert_eq!(agent.messages().len(), 4, "prompt, answer, steer, answer");
}

#[tokio::test]
async fn rewinding_drops_the_last_prompt_and_its_answer() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![text_turn("first answer"), text_turn("second answer")]);
    let mut agent = agent(provider, dir.path(), config());

    agent.run("first", &mut |_| {}).await.expect("run");
    agent.run("second", &mut |_| {}).await.expect("run");
    assert_eq!(agent.messages().len(), 4);

    assert!(agent.rewind_last_prompt());
    assert_eq!(agent.messages().len(), 2);
    assert_eq!(agent.messages()[0].content, "first");
    assert_eq!(agent.messages()[1].content, "first answer");

    assert!(agent.rewind_last_prompt());
    assert!(agent.messages().is_empty());
    assert!(!agent.rewind_last_prompt(), "there is nothing left to undo");
}

#[cfg(unix)]
fn notify_script(dir: &std::path::Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = dir.join("notify.sh");
    std::fs::write(&path, "#!/bin/sh\ncat >> \"$1\"\n").expect("script");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("executable");
    path
}

#[cfg(unix)]
fn read_payloads(path: &std::path::Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("payload is JSON"))
        .collect()
}
#[cfg(unix)]
#[tokio::test]
async fn a_finished_turn_runs_the_notify_program_once_with_the_documented_payload() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = dir.path().join("payload.jsonl");
    let notify = Arc::new(
        Notify::from_program(
            Some(&notify_script(dir.path()).display().to_string()),
            vec![out.display().to_string()],
        )
        .expect("notifier"),
    );
    let provider = Scripted::new(vec![text_turn("hello")]);
    let mut agent = agent(provider, dir.path(), config())
        .with_notify(Arc::clone(&notify), "mock:model")
        .with_session_id("session-9");

    let outcome = agent.run("hi", &mut |_| {}).await.expect("run");
    assert_eq!(outcome.steps, 1);
    assert!(notify.wait_for(std::time::Duration::from_secs(10)));

    let payloads = read_payloads(&out);
    assert_eq!(payloads.len(), 1, "one program run per turn");
    let payload = &payloads[0];
    assert_eq!(payload["event"], "turn_end");
    assert_eq!(payload["session"], "session-9");
    assert_eq!(payload["model"], "mock:model");
    assert_eq!(
        payload["cwd"],
        dir.path()
            .canonicalize()
            .expect("canonical")
            .display()
            .to_string()
    );
    assert_eq!(payload["reason"], "end_turn");
    assert_eq!(payload["steps"], 1);
    assert_eq!(payload["tool_calls"], 0);
    assert_eq!(payload["answer"], "hello");
}

#[cfg(unix)]
#[tokio::test]
async fn a_provider_failure_is_announced_as_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = dir.path().join("payload.jsonl");
    let notify = Arc::new(
        Notify::from_program(
            Some(&notify_script(dir.path()).display().to_string()),
            vec![out.display().to_string()],
        )
        .expect("notifier"),
    );
    let mut agent = Agent::new(
        Box::new(Failing),
        ToolContext::new(dir.path()).expect("workspace"),
        config(),
    )
    .with_notify(Arc::clone(&notify), "test");

    let error = agent
        .run("hi", &mut |_| {})
        .await
        .expect_err("the provider fails");
    assert!(
        error.to_string().contains("could not be reached"),
        "{error}"
    );
    assert!(notify.wait_for(std::time::Duration::from_secs(10)));

    let payloads = read_payloads(&out);
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0]["event"], "turn_end");
    assert_eq!(payloads[0]["reason"], "error");
    assert_eq!(payloads[0]["steps"], 1);
    assert_eq!(payloads[0]["tool_calls"], 0);
    assert_eq!(payloads[0]["answer"], "");
}

#[cfg(unix)]
#[tokio::test]
async fn a_cancel_before_the_first_turn_is_not_announced() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = dir.path().join("payload.jsonl");
    let notify = Arc::new(
        Notify::from_program(
            Some(&notify_script(dir.path()).display().to_string()),
            vec![out.display().to_string()],
        )
        .expect("notifier"),
    );
    let cancel = CancelHandle::new();
    cancel.cancel();
    let provider = Scripted::new(vec![text_turn("never reached")]);
    let mut agent = agent(provider, dir.path(), config())
        .with_cancel(cancel)
        .with_notify(Arc::clone(&notify), "test");

    let outcome = agent.run("hi", &mut |_| {}).await.expect("run");
    assert_eq!(outcome.steps, 0);
    assert!(
        notify.wait_for(std::time::Duration::from_millis(200)),
        "nothing is pending"
    );
    assert!(
        !out.exists(),
        "an interrupted-before-first-turn run does not fire"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn compaction_alone_does_not_notify() {
    let dir = tempfile::tempdir().expect("tempdir");
    let out = dir.path().join("payload.jsonl");
    let notify = Arc::new(
        Notify::from_program(
            Some(&notify_script(dir.path()).display().to_string()),
            vec![out.display().to_string()],
        )
        .expect("notifier"),
    );
    let provider = Scripted::new(Vec::new());
    let mut agent = agent(provider, dir.path(), config())
        .with_history(vec![
            niki_provider::Message::user("old question"),
            niki_provider::Message::assistant("old answer"),
        ])
        .with_notify(Arc::clone(&notify), "test");

    let _ = agent.compact_without_model(0);
    assert!(notify.wait_for(std::time::Duration::from_millis(200)));
    assert!(!out.exists(), "compaction is not a turn");
}

/// A provider whose first request fails, for the failed-turn path.
struct Failing;

impl Provider for Failing {
    fn name(&self) -> &str {
        "failing"
    }

    fn stream(
        &self,
        _request: ChatRequest,
    ) -> BoxStream<'static, Result<StreamEvent, ProviderError>> {
        futures_util::stream::once(async {
            Err(ProviderError::Transport {
                provider: "failing".to_owned(),
                detail: "down".to_owned(),
            })
        })
        .boxed()
    }
}
