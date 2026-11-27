//! MCP tools through the real code paths: `niki_mcp::load` over a parsed
//! `[mcp_servers]` table, the agent turn loop, and the screen's worker.
//!
//! The server is `src/bin/mcp-fixture.rs`, so the handshake, the tool call and
//! the approval prompt all run against a child process. No network (RULES.md 8).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use niki_agent::{Agent, AgentConfig, AgentEvent, AllowList, Approval, ApprovalRequest, Approver};
use niki_config::Config;
use niki_mcp::load;
use niki_policy::Mode;
use niki_provider::{ChatRequest, Provider, ProviderError, StopReason, StreamEvent};
use niki_tools::{Registry, Tool, ToolContext};
use niki_tui_live::{App, RowKind, SessionInfo, TurnState, WorkerEvent, WorkerHandle};

/// A provider that replays one scripted turn per request.
struct Scripted {
    turns: Arc<Mutex<VecDeque<Vec<StreamEvent>>>>,
}

impl Scripted {
    fn new(turns: Vec<Vec<StreamEvent>>) -> Self {
        Self {
            turns: Arc::new(Mutex::new(turns.into())),
        }
    }
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

/// The config a person would write to reach the fixture server.
fn fixture_config(name: &str) -> Config {
    let command = serde_json::to_string(env!("CARGO_BIN_EXE_mcp-fixture")).expect("quote");
    Config::from_str(&format!("[mcp_servers.{name}]\ncommand = {command}\n")).expect("config")
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

fn registry(tools: Vec<niki_mcp::ToolBridge>) -> Registry {
    let mut registry = Registry::new();
    for tool in tools {
        registry.register(tool);
    }
    registry
}

#[tokio::test]
async fn a_server_that_fails_to_start_does_not_hide_the_others() {
    let command = serde_json::to_string(env!("CARGO_BIN_EXE_mcp-fixture")).expect("quote");
    let config = Config::from_str(&format!(
        "[mcp_servers.broken]\ncommand = \"/nonexistent/niki-mcp-server\"\n\n\
         [mcp_servers.fixture]\ncommand = {command}\n"
    ))
    .expect("config");

    let (tools, errors) = load(&config.mcp_servers).await;

    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].to_string().contains("broken"), "{}", errors[0]);
    let names: Vec<&str> = tools.iter().map(Tool::name).collect();
    assert!(names.contains(&"mcp__fixture__echo"), "{names:?}");
}

#[tokio::test]
async fn a_mutating_mcp_tool_asks_before_it_runs() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config = fixture_config("fixture");
    let (tools, errors) = load(&config.mcp_servers).await;
    assert!(errors.is_empty(), "{errors:?}");

    let provider = Scripted::new(vec![
        tool_turn(
            "call-1",
            "mcp__fixture__echo",
            r#"{"message":"hi from the bridge"}"#,
        ),
        text_turn("done"),
    ]);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&seen);
    let approver: Approver = Box::new(move |request: ApprovalRequest| {
        recorded.lock().expect("lock").push(request);
        Box::pin(async { Approval::Once })
    });
    let ctx = ToolContext::new(workspace.path()).expect("workspace");
    let mut agent = Agent::new(
        Box::new(provider),
        ctx,
        AgentConfig {
            model: "test-model".to_owned(),
            ..AgentConfig::default()
        },
    )
    .with_registry(registry(tools))
    .with_policy(Mode::Manual, AllowList::parse(""))
    .with_approver(approver);

    let mut events = Vec::new();
    let outcome = agent
        .run("say hi", &mut |event| events.push(event.clone()))
        .await
        .expect("run");

    assert_eq!(outcome.tool_calls, 1);
    let requests = seen.lock().expect("lock");
    assert_eq!(requests.len(), 1, "one prompt for one gated call");
    assert!(
        requests[0].command.starts_with("mcp__fixture__echo"),
        "{}",
        requests[0].command
    );
    assert_eq!(requests[0].mode, Mode::Manual);

    let tool_end = events
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
    assert_eq!(tool_end.0, "mcp__fixture__echo");
    assert!(!tool_end.2, "{}", tool_end.1);
    assert_eq!(tool_end.1, "hi from the bridge");
}

#[test]
fn the_worker_registers_a_configured_tool_and_asks_the_screen() {
    let workspace = tempfile::tempdir().expect("workspace");
    let config = fixture_config("fixture");
    let runtime = runtime();
    let (tools, errors) = runtime.block_on(load(&config.mcp_servers));
    assert!(errors.is_empty(), "{errors:?}");
    let tools = Arc::new(tools);

    let turns = Arc::new(Mutex::new(VecDeque::from(vec![
        tool_turn(
            "call-1",
            "mcp__fixture__echo",
            r#"{"message":"through the screen"}"#,
        ),
        text_turn("done"),
    ])));
    let build_turns = Arc::clone(&turns);
    let ctx = ToolContext::new(workspace.path()).expect("workspace");

    let mut worker =
        WorkerHandle::spawn_with_runtime("mock:mock-model", Mode::Manual, runtime, move |parts| {
            let mut registry = Registry::new();
            for tool in tools.iter() {
                registry.register(tool.clone());
            }
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
            .with_registry(registry)
            .with_policy(parts.mode, AllowList::parse(""))
            .with_cancel(parts.cancel)
            .with_approver(parts.approver))
        });

    let mut app = App::new(SessionInfo {
        model: "mock:mock-model".to_owned(),
        mode: Mode::Manual,
        cwd: workspace.path().to_path_buf(),
    });
    assert!(worker.submit("say it"));

    let mut asked = 0;
    loop {
        match worker.recv_timeout(Duration::from_secs(10)) {
            Some(WorkerEvent::Agent(event)) => app.apply(event),
            Some(WorkerEvent::Approval { request, reply }) => {
                asked += 1;
                assert!(
                    request.command.starts_with("mcp__fixture__echo"),
                    "{}",
                    request.command
                );
                app.request_approval(request);
                assert_eq!(app.status.turn, TurnState::Waiting);
                reply.send(Approval::Once).expect("answer");
            }
            Some(WorkerEvent::Finished(result)) => {
                app.finish(result.map_err(|message| message.to_string()));
                break;
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
            Some(WorkerEvent::Question { .. }) => panic!("this turn asks no question"),
            None => panic!("the worker went quiet before the turn finished"),
        }
    }

    assert_eq!(asked, 1, "a mutating tool asks once");
    assert_eq!(app.status.turn, TurnState::Idle);
    let result = app
        .transcript
        .rows()
        .iter()
        .find(|row| row.kind == RowKind::ToolResult)
        .expect("the tool result row");
    assert_eq!(result.title.as_deref(), Some("mcp__fixture__echo"));
    assert_eq!(result.text, "through the screen");
}
