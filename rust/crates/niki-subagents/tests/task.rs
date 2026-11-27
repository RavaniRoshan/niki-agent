use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use niki_agent::{Agent, AgentConfig, CancelHandle};
use niki_provider::{ChatRequest, Provider, ProviderError, StopReason, StreamEvent};
use niki_subagents::SubagentTool;
use niki_tools::{Registry, Tool, ToolContext, ToolError, ToolOutput};
use serde_json::{Value, json};

/// A provider that replays one scripted turn per request and records what it
/// was asked. No network, so the tool is testable anywhere (RULES.md 8).
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
        StreamEvent::Done(StopReason::ToolUse),
    ]
}

/// A tool the child must never reach, standing in for `task` itself.
struct FakeTask;

impl Tool for FakeTask {
    fn name(&self) -> &str {
        "task"
    }

    fn description(&self) -> &str {
        "delegates again"
    }

    fn schema(&self) -> Value {
        json!({"type": "object"})
    }

    async fn run(&self, _args: Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        Ok(ToolOutput::new("a child must not delegate"))
    }
}

fn tool<F>(dir: &std::path::Path, child: Scripted, tools: F) -> SubagentTool
where
    F: Fn() -> Registry + Send + Sync + 'static,
{
    let root = dir.to_path_buf();
    SubagentTool::new(tools, move |registry, budget| {
        let ctx = ToolContext::new(&root).map_err(|error| error.to_string())?;
        let settings = AgentConfig {
            model: "child-model".to_owned(),
            max_steps: budget,
            ..AgentConfig::default()
        };
        Ok(Agent::new(Box::new(child.clone()), ctx, settings).with_registry(registry))
    })
}

fn args() -> Value {
    json!({
        "description": "inspect the tree",
        "prompt": "list the workspace and answer with one sentence"
    })
}

#[tokio::test]
async fn the_childs_answer_is_returned_with_a_summary() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = ToolContext::new(dir.path()).expect("workspace");
    let child = Scripted::new(vec![text_turn("the tree has three files")]);
    let tool = tool(dir.path(), child, Registry::new);

    let output = tool.run(args(), &ctx).await.expect("run");

    assert!(
        output.text.contains("the tree has three files"),
        "{}",
        output.text
    );
    assert!(
        output
            .text
            .contains("subagent: inspect the tree — 1 steps, 0 tool calls"),
        "{}",
        output.text
    );
    assert!(!output.truncated);
}

#[tokio::test]
async fn the_child_cannot_see_the_task_tool() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = ToolContext::new(dir.path()).expect("workspace");
    let child = Scripted::new(vec![text_turn("done")]);
    let observed = child.clone();
    let tool = tool(dir.path(), child, || {
        let mut registry = Registry::new();
        registry.register(FakeTask);
        registry
    });

    tool.run(args(), &ctx).await.expect("run");

    let requests = observed.requests();
    assert_eq!(requests.len(), 1);
    let names: Vec<&str> = requests[0]
        .tools
        .iter()
        .map(|spec| spec.name.as_str())
        .collect();
    assert!(!names.contains(&"task"), "{names:?}");
    assert!(names.contains(&"read_file"), "{names:?}");
}

#[tokio::test]
async fn running_out_of_budget_returns_what_the_child_produced() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = ToolContext::new(dir.path()).expect("workspace");
    let child = Scripted::new(vec![
        tool_turn("c1", "list_dir", r#"{"path":"."}"#),
        tool_turn("c2", "list_dir", r#"{"path":"."}"#),
        text_turn("never reached"),
    ]);
    let tool = tool(dir.path(), child, Registry::new).with_step_budget(2);

    let output = tool.run(args(), &ctx).await.expect("run");

    assert!(
        output
            .text
            .contains("subagent: inspect the tree — 2 steps, 2 tool calls"),
        "{}",
        output.text
    );
    assert!(
        output.text.contains("step budget of 2 ran out"),
        "{}",
        output.text
    );
}

#[tokio::test]
async fn a_cancelled_child_returns_promptly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = ToolContext::new(dir.path()).expect("workspace");
    let cancel = CancelHandle::new();
    cancel.cancel();
    let child = Scripted::new(vec![text_turn("never reached")]);
    let observed = child.clone();
    let tool = tool(dir.path(), child, Registry::new).with_cancel(cancel);

    let started = Instant::now();
    let output = tool.run(args(), &ctx).await.expect("run");
    let elapsed = started.elapsed();

    assert!(elapsed < Duration::from_secs(2), "took {elapsed:?}");
    assert!(output.text.contains("cancelled"), "{}", output.text);
    assert!(
        observed.requests().is_empty(),
        "the child never called the model"
    );
}

#[tokio::test]
async fn a_missing_description_is_an_argument_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = ToolContext::new(dir.path()).expect("workspace");
    let child = Scripted::new(Vec::new());
    let tool = tool(dir.path(), child, Registry::new);

    let error = tool
        .run(json!({"prompt": "do it"}), &ctx)
        .await
        .expect_err("no label");

    assert!(
        matches!(error, ToolError::InvalidArguments(ref message) if message.contains("description"))
    );
}
