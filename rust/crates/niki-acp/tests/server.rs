//! End-to-end checks of the editor protocol over in-memory pipes.
//!
//! A scripted provider drives the real turn loop, so no test reaches a network
//! (RULES.md 8). The client side of the pipes sends protocol frames and reads
//! the server's frames back, including permission requests it answers.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use niki_acp::{AgentFactory, ServerError, ServerOptions, SessionSeed, serve};
use niki_agent::{Agent, AgentConfig, AllowList};
use niki_provider::{ChatRequest, Provider, ProviderError, StopReason, StreamEvent};
use niki_tools::{Registry, Tool, ToolContext, ToolError, ToolOutput};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf};

/// A provider that replays one scripted turn per request.
struct Scripted {
    turns: Mutex<VecDeque<Vec<StreamEvent>>>,
    seen: Mutex<Vec<ChatRequest>>,
    delay: Duration,
}

impl Scripted {
    fn new(turns: Vec<Vec<StreamEvent>>) -> Arc<Self> {
        Self::with_delay(turns, Duration::ZERO)
    }

    fn with_delay(turns: Vec<Vec<StreamEvent>>, delay: Duration) -> Arc<Self> {
        Arc::new(Self {
            turns: Mutex::new(turns.into()),
            seen: Mutex::new(Vec::new()),
            delay,
        })
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
        let delay = self.delay;
        futures_util::stream::iter(events)
            .then(move |event| async move {
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
                Ok::<StreamEvent, ProviderError>(event)
            })
            .boxed()
    }
}

struct Shared(Arc<dyn Provider>);

impl Provider for Shared {
    fn name(&self) -> &str {
        self.0.name()
    }

    fn stream(
        &self,
        request: ChatRequest,
    ) -> BoxStream<'static, Result<StreamEvent, ProviderError>> {
        self.0.stream(request)
    }
}

/// A provider whose one request fails before it streams anything.
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
                detail: "connection reset".to_owned(),
            })
        })
        .boxed()
    }
}

/// A tool the policy must ask about before it runs.
struct Gated {
    runs: Arc<AtomicUsize>,
}

impl Tool for Gated {
    fn name(&self) -> &str {
        "gated"
    }

    fn description(&self) -> &str {
        "A tool that needs approval."
    }

    fn schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    fn requires_approval(&self) -> bool {
        true
    }

    async fn run(&self, _args: Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput::new("ran"))
    }
}

/// A tool whose arguments carry a plan.
struct Todo;

impl Tool for Todo {
    fn name(&self) -> &str {
        "write_todos"
    }

    fn description(&self) -> &str {
        "Record the plan."
    }

    fn schema(&self) -> Value {
        json!({ "type": "object", "properties": { "todos": { "type": "array" } } })
    }

    async fn run(&self, _args: Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        Ok(ToolOutput::new("plan noted"))
    }
}

fn factory<P>(
    provider: Arc<P>,
    build: impl Fn() -> Registry + Send + Sync + 'static,
) -> AgentFactory
where
    P: Provider + 'static,
{
    let provider: Arc<dyn Provider> = provider;
    Box::new(move |seed: SessionSeed| {
        let ctx = ToolContext::new(seed.cwd.as_path()).map_err(|error| error.to_string())?;
        let settings = AgentConfig {
            model: "mock-model".to_owned(),
            ..AgentConfig::default()
        };
        Ok(
            Agent::new(Box::new(Shared(Arc::clone(&provider))), ctx, settings)
                .with_registry(build())
                .with_policy(seed.mode, AllowList::parse(""))
                .with_cancel(seed.cancel)
                .with_approver(seed.approver),
        )
    })
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

/// One test's client side of the connection.
struct Harness {
    write: WriteHalf<DuplexStream>,
    read: BufReader<ReadHalf<DuplexStream>>,
    notifications: Vec<Value>,
    permissions: Vec<Value>,
    permission: String,
    next_id: u64,
    _task: tokio::task::JoinHandle<Result<(), ServerError>>,
}

impl Harness {
    fn start(factory: AgentFactory, options: ServerOptions) -> Self {
        let (client, server) = tokio::io::duplex(1 << 20);
        let (client_read, client_write) = tokio::io::split(client);
        let (server_read, server_write) = tokio::io::split(server);
        let task = tokio::spawn(serve(server_read, server_write, factory, options));
        Self {
            write: client_write,
            read: BufReader::new(client_read),
            notifications: Vec::new(),
            permissions: Vec::new(),
            permission: "approve".to_owned(),
            next_id: 0,
            _task: task,
        }
    }

    async fn send(&mut self, frame: Value) {
        let mut line = serde_json::to_vec(&frame).expect("frame");
        line.push(b'\n');
        self.write.write_all(&line).await.expect("write");
        self.write.flush().await.expect("flush");
    }

    async fn send_raw(&mut self, text: &str) {
        self.write.write_all(text.as_bytes()).await.expect("write");
        self.write.flush().await.expect("flush");
    }

    fn take_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    async fn next_frame(&mut self) -> Value {
        let mut line = String::new();
        let read = tokio::time::timeout(Duration::from_secs(5), self.read.read_line(&mut line))
            .await
            .expect("a frame within five seconds")
            .expect("read");
        assert!(read > 0, "the server closed the connection");
        serde_json::from_str(line.trim()).expect("json frame")
    }

    async fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.take_id();
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
            .await;
        self.read_until(id).await
    }

    async fn read_until(&mut self, id: u64) -> Value {
        loop {
            let frame = self.next_frame().await;
            if frame["method"] == json!("session/request_permission") {
                self.permissions.push(frame.clone());
                let request_id = frame["id"].clone();
                let result = json!({
                    "outcome": { "outcome": "selected", "optionId": self.permission },
                });
                self.send(json!({ "jsonrpc": "2.0", "id": request_id, "result": result }))
                    .await;
                continue;
            }
            if frame.get("method").is_some() {
                self.notifications.push(frame);
                continue;
            }
            if frame["id"] == json!(id) {
                return frame;
            }
        }
    }

    fn mark(&self) -> usize {
        self.notifications.len()
    }

    fn updates_since(&self, start: usize, kind: &str) -> Vec<&Value> {
        self.notifications[start..]
            .iter()
            .filter(|frame| frame["params"]["update"]["sessionUpdate"] == json!(kind))
            .collect()
    }

    fn update_field(&self, start: usize, kind: &str, field: &str) -> Value {
        let updates = self.updates_since(start, kind);
        assert!(!updates.is_empty(), "no {kind} update after {start}");
        updates[0]["params"]["update"][field].clone()
    }
}

async fn new_session(harness: &mut Harness, cwd: &Path) -> String {
    let response = harness
        .call(
            "session/new",
            json!({ "cwd": cwd.to_str().expect("utf-8"), "mcpServers": [] }),
        )
        .await;
    response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_owned()
}

async fn run_prompt(harness: &mut Harness, session: &str, text: &str) -> Value {
    harness
        .call(
            "session/prompt",
            json!({
                "sessionId": session,
                "prompt": [{ "type": "text", "text": text }],
            }),
        )
        .await
}

#[tokio::test]
async fn initialize_echoes_the_version_and_advertises_text_prompts() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = dir.path().join("sessions.db");
    let provider = Scripted::new(Vec::new());
    let mut harness = Harness::start(
        factory(provider, Registry::new),
        ServerOptions::new().with_store(&store),
    );

    let response = harness
        .call(
            "initialize",
            json!({ "protocolVersion": 1, "clientCapabilities": {} }),
        )
        .await;

    assert_eq!(response["result"]["protocolVersion"], json!(1));
    assert_eq!(
        response["result"]["agentCapabilities"]["loadSession"],
        json!(true)
    );
    assert_eq!(
        response["result"]["agentCapabilities"]["promptCapabilities"]["image"],
        json!(false)
    );
    assert_eq!(response["result"]["agentInfo"]["name"], json!("niki"));
}

#[tokio::test]
async fn a_prompt_streams_the_answer_and_session_load_replays_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = dir.path().join("sessions.db");
    let provider = Scripted::new(vec![text_turn("hello there")]);
    let mut harness = Harness::start(
        factory(Arc::clone(&provider), Registry::new),
        ServerOptions::new().with_store(&store),
    );
    harness
        .call("initialize", json!({ "protocolVersion": 1 }))
        .await;
    let session = new_session(&mut harness, dir.path()).await;

    let start = harness.mark();
    let response = run_prompt(&mut harness, &session, "hi").await;

    assert_eq!(response["result"]["stopReason"], json!("end_turn"));
    assert_eq!(
        harness.update_field(start, "agent_message_chunk", "content")["text"],
        json!("hello there")
    );
    assert!(
        harness
            .updates_since(start, "user_message_chunk")
            .is_empty(),
        "a live prompt is not echoed back as a user chunk"
    );
    let requests = provider.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].messages[0].content, "hi");

    let replay = harness.mark();
    let loaded = harness
        .call(
            "session/load",
            json!({
                "cwd": dir.path().to_str().expect("utf-8"),
                "sessionId": session,
                "mcpServers": [],
            }),
        )
        .await;
    assert_eq!(loaded["result"]["modes"]["currentModeId"], json!("manual"));
    assert_eq!(
        harness.update_field(replay, "user_message_chunk", "content")["text"],
        json!("hi")
    );
    let chunks = harness.updates_since(replay, "agent_message_chunk");
    assert_eq!(chunks.len(), 1);
    assert_eq!(
        chunks[0]["params"]["update"]["content"]["text"],
        json!("hello there")
    );
    assert!(chunks[0]["params"]["update"]["messageId"].is_string());
}

#[tokio::test]
async fn a_workspace_tool_runs_and_streams_tool_call_updates() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = dir.path().join("sessions.db");
    let provider = Scripted::new(vec![
        tool_turn("t1", "write_file", r#"{"path":"a.txt","content":"hi"}"#),
        text_turn("done"),
    ]);
    let mut harness = Harness::start(
        factory(provider, Registry::new),
        ServerOptions::new().with_store(&store),
    );
    let session = new_session(&mut harness, dir.path()).await;

    let start = harness.mark();
    let response = run_prompt(&mut harness, &session, "write it").await;

    assert_eq!(response["result"]["stopReason"], json!("end_turn"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).expect("file"),
        "hi"
    );

    let calls = harness.updates_since(start, "tool_call");
    assert_eq!(calls.len(), 1);
    let update = &calls[0]["params"]["update"];
    assert_eq!(update["title"], json!("Write `a.txt`"));
    assert_eq!(update["kind"], json!("edit"));
    assert_eq!(update["status"], json!("pending"));
    assert_eq!(update["rawInput"]["path"], json!("a.txt"));
    let call_id = update["toolCallId"].as_str().expect("call id");
    assert_eq!(call_id, "call-0");

    let updates = harness.updates_since(start, "tool_call_update");
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0]["params"]["update"]["toolCallId"], json!(call_id));
    assert_eq!(updates[0]["params"]["update"]["status"], json!("completed"));
    assert_eq!(
        updates[0]["params"]["update"]["content"][0]["content"]["text"],
        json!("wrote 2 bytes to a.txt")
    );

    let replay = harness.mark();
    harness
        .call(
            "session/load",
            json!({
                "cwd": dir.path().to_str().expect("utf-8"),
                "sessionId": session,
                "mcpServers": [],
            }),
        )
        .await;
    let replayed = harness.updates_since(replay, "tool_call");
    assert_eq!(replayed.len(), 1);
    assert_eq!(
        replayed[0]["params"]["update"]["title"],
        json!("Write `a.txt`")
    );
    let replayed_updates = harness.updates_since(replay, "tool_call_update");
    assert_eq!(replayed_updates.len(), 1);
    assert_eq!(
        replayed_updates[0]["params"]["update"]["status"],
        json!("completed")
    );
}

#[tokio::test]
async fn a_gated_tool_asks_the_editor_and_always_allow_stops_the_second_ask() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runs = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&runs);
    let provider = Scripted::new(vec![
        tool_turn("g1", "gated", "{}"),
        text_turn("first"),
        tool_turn("g2", "gated", "{}"),
        text_turn("second"),
    ]);
    let mut harness = Harness::start(
        factory(provider, move || {
            let mut registry = Registry::new();
            registry.register(Gated {
                runs: Arc::clone(&counted),
            });
            registry
        }),
        ServerOptions::new(),
    );
    harness.permission = "approve_always".to_owned();
    let session = new_session(&mut harness, dir.path()).await;

    run_prompt(&mut harness, &session, "run it").await;
    assert_eq!(harness.permissions.len(), 1);
    let request = &harness.permissions[0];
    assert_eq!(request["method"], json!("session/request_permission"));
    assert_eq!(request["params"]["sessionId"], json!(session));
    assert_eq!(request["params"]["toolCall"]["toolCallId"], json!("call-0"));
    assert_eq!(request["params"]["toolCall"]["title"], json!("gated"));
    let kinds: Vec<&str> = request["params"]["options"]
        .as_array()
        .expect("options")
        .iter()
        .filter_map(|option| option["kind"].as_str())
        .collect();
    assert_eq!(kinds, ["allow_once", "reject_once", "allow_always"]);

    run_prompt(&mut harness, &session, "again").await;
    assert_eq!(harness.permissions.len(), 1, "always-allow was remembered");
    assert_eq!(runs.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_rejected_permission_refuses_the_tool_and_keeps_the_turn() {
    let dir = tempfile::tempdir().expect("tempdir");
    let runs = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&runs);
    let provider = Scripted::new(vec![
        tool_turn("g1", "gated", "{}"),
        text_turn("understood"),
    ]);
    let mut harness = Harness::start(
        factory(provider, move || {
            let mut registry = Registry::new();
            registry.register(Gated {
                runs: Arc::clone(&counted),
            });
            registry
        }),
        ServerOptions::new(),
    );
    harness.permission = "reject".to_owned();
    let session = new_session(&mut harness, dir.path()).await;

    let start = harness.mark();
    let response = run_prompt(&mut harness, &session, "run it").await;

    assert_eq!(response["result"]["stopReason"], json!("end_turn"));
    assert_eq!(runs.load(Ordering::SeqCst), 0);
    let updates = harness.updates_since(start, "tool_call_update");
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0]["params"]["update"]["status"], json!("failed"));
    let output = updates[0]["params"]["update"]["content"][0]["content"]["text"]
        .as_str()
        .expect("content");
    assert!(output.starts_with("refused:"), "{output}");
    assert_eq!(
        harness.update_field(start, "agent_message_chunk", "content")["text"],
        json!("understood")
    );
}

#[tokio::test]
async fn cancel_stops_the_turn_and_keeps_the_partial_transcript() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = dir.path().join("sessions.db");
    let turn = vec![
        StreamEvent::TextDelta("one".to_owned()),
        StreamEvent::TextDelta("two".to_owned()),
        StreamEvent::TextDelta("three".to_owned()),
        StreamEvent::Done(StopReason::EndTurn),
    ];
    let provider = Scripted::with_delay(vec![turn], Duration::from_millis(40));
    let mut harness = Harness::start(
        factory(provider, Registry::new),
        ServerOptions::new().with_store(&store),
    );
    let session = new_session(&mut harness, dir.path()).await;

    let id = harness.take_id();
    harness
        .send(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "session/prompt",
            "params": { "sessionId": session, "prompt": [{ "type": "text", "text": "slow" }] },
        }))
        .await;
    loop {
        let frame = harness.next_frame().await;
        if frame["params"]["update"]["sessionUpdate"] == json!("agent_message_chunk") {
            break;
        }
    }
    harness
        .send(json!({
            "jsonrpc": "2.0",
            "method": "session/cancel",
            "params": { "sessionId": session },
        }))
        .await;
    let response = harness.read_until(id).await;
    assert_eq!(response["result"]["stopReason"], json!("cancelled"));

    let replay = harness.mark();
    harness
        .call(
            "session/load",
            json!({
                "cwd": dir.path().to_str().expect("utf-8"),
                "sessionId": session,
                "mcpServers": [],
            }),
        )
        .await;
    let chunks = harness.updates_since(replay, "agent_message_chunk");
    assert!(!chunks.is_empty(), "the partial answer is kept");
    let text = chunks
        .iter()
        .map(|chunk| {
            chunk["params"]["update"]["content"]["text"]
                .as_str()
                .unwrap_or("")
        })
        .collect::<String>();
    assert!(text.starts_with("one"), "{text}");
}

#[tokio::test]
async fn session_load_rejects_a_mismatched_working_directory_and_an_unknown_session() {
    let dir = tempfile::tempdir().expect("tempdir");
    let other = tempfile::tempdir().expect("tempdir");
    let store = dir.path().join("sessions.db");
    let provider = Scripted::new(vec![text_turn("hello")]);
    let mut harness = Harness::start(
        factory(provider, Registry::new),
        ServerOptions::new().with_store(&store),
    );
    let session = new_session(&mut harness, dir.path()).await;

    let mismatch = harness
        .call(
            "session/load",
            json!({
                "cwd": other.path().to_str().expect("utf-8"),
                "sessionId": session,
                "mcpServers": [],
            }),
        )
        .await;
    assert_eq!(mismatch["error"]["code"], json!(-32602));
    assert_eq!(
        mismatch["error"]["data"]["cwd"],
        json!("must match the working directory used to create the session")
    );

    let missing = harness
        .call(
            "session/load",
            json!({
                "cwd": dir.path().to_str().expect("utf-8"),
                "sessionId": "no-such-session",
                "mcpServers": [],
            }),
        )
        .await;
    assert_eq!(missing["error"]["code"], json!(-32002));
    assert_eq!(missing["error"]["data"]["uri"], json!("no-such-session"));
}

#[tokio::test]
async fn modes_and_config_options_can_be_changed_and_are_validated() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(Vec::new());
    let mut harness = Harness::start(factory(provider, Registry::new), ServerOptions::new());
    let session = new_session(&mut harness, dir.path()).await;

    let mode = harness
        .call(
            "session/set_mode",
            json!({ "sessionId": session, "modeId": "auto" }),
        )
        .await;
    assert_eq!(mode["result"], json!({}));

    let option = harness
        .call(
            "session/set_config_option",
            json!({ "sessionId": session, "configId": "mode", "value": "yolo" }),
        )
        .await;
    assert_eq!(
        option["result"]["configOptions"][0]["currentValue"],
        json!("yolo")
    );

    let invalid = harness
        .call(
            "session/set_mode",
            json!({ "sessionId": session, "modeId": "fast" }),
        )
        .await;
    assert_eq!(invalid["error"]["code"], json!(-32602));
    assert_eq!(invalid["error"]["message"], json!("Invalid mode: fast"));

    let unknown = harness
        .call(
            "session/set_config_option",
            json!({ "sessionId": session, "configId": "theme", "value": "dark" }),
        )
        .await;
    assert_eq!(unknown["error"]["code"], json!(-32602));
    assert_eq!(
        unknown["error"]["message"],
        json!("Unknown config option: theme")
    );

    let boolean = harness
        .call(
            "session/set_config_option",
            json!({ "sessionId": session, "configId": "mode", "value": true, "type": "boolean" }),
        )
        .await;
    assert_eq!(boolean["error"]["code"], json!(-32602));
}

#[tokio::test]
async fn unknown_methods_and_unknown_sessions_are_reported_not_guessed() {
    let provider = Scripted::new(Vec::new());
    let mut harness = Harness::start(factory(provider, Registry::new), ServerOptions::new());

    let unknown = harness
        .call("session/delete", json!({ "sessionId": "x" }))
        .await;
    assert_eq!(unknown["error"]["code"], json!(-32601));
    assert_eq!(unknown["error"]["data"]["method"], json!("session/delete"));

    let invalid = harness.call("session/new", json!({})).await;
    assert_eq!(invalid["error"]["code"], json!(-32602));

    let missing = harness
        .call(
            "session/prompt",
            json!({ "sessionId": "missing", "prompt": [{ "type": "text", "text": "hi" }] }),
        )
        .await;
    assert_eq!(missing["error"]["code"], json!(-32002));
    assert_eq!(missing["error"]["data"]["uri"], json!("missing"));

    let image = harness
        .call(
            "session/prompt",
            json!({
                "sessionId": "missing",
                "prompt": [{ "type": "image", "data": "AAAA", "mimeType": "image/png" }],
            }),
        )
        .await;
    assert_eq!(image["error"]["code"], json!(-32602));

    let relative = harness
        .call("session/new", json!({ "cwd": "relative/dir" }))
        .await;
    assert_eq!(relative["error"]["code"], json!(-32602));
    assert_eq!(
        relative["error"]["data"]["cwd"],
        json!("must be an existing directory")
    );
}

#[tokio::test]
async fn a_todo_tool_streams_a_plan_update() {
    let dir = tempfile::tempdir().expect("tempdir");
    let provider = Scripted::new(vec![
        tool_turn(
            "p1",
            "write_todos",
            r#"{"todos":[{"content":"read","status":"completed"},{"content":"write","status":"in_progress"}]}"#,
        ),
        text_turn("planned"),
    ]);
    let mut harness = Harness::start(
        factory(provider, || {
            let mut registry = Registry::new();
            registry.register(Todo);
            registry
        }),
        ServerOptions::new(),
    );
    let session = new_session(&mut harness, dir.path()).await;

    let start = harness.mark();
    run_prompt(&mut harness, &session, "plan it").await;

    let calls = harness.updates_since(start, "tool_call");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["params"]["update"]["title"], json!("write_todos"));
    let plans = harness.updates_since(start, "plan");
    assert_eq!(plans.len(), 1);
    let entries = &plans[0]["params"]["update"]["entries"];
    assert_eq!(entries[0]["content"], json!("read"));
    assert_eq!(entries[0]["status"], json!("completed"));
    assert_eq!(entries[1]["status"], json!("in_progress"));
    assert_eq!(entries[1]["priority"], json!("medium"));

    let call_at = harness.notifications[start..]
        .iter()
        .position(|frame| frame["params"]["update"]["sessionUpdate"] == json!("tool_call"))
        .expect("call");
    let plan_at = harness.notifications[start..]
        .iter()
        .position(|frame| frame["params"]["update"]["sessionUpdate"] == json!("plan"))
        .expect("plan");
    assert!(
        call_at < plan_at,
        "the plan follows the call that carries it"
    );
}

#[tokio::test]
async fn a_provider_failure_is_reported_with_the_provider_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = dir.path().join("sessions.db");
    let mut harness = Harness::start(
        factory(Arc::new(Failing), Registry::new),
        ServerOptions::new().with_store(&store),
    );
    let session = new_session(&mut harness, dir.path()).await;

    let response = run_prompt(&mut harness, &session, "hi").await;
    assert_eq!(response["error"]["code"], json!(-32603));
    let details = response["error"]["data"]["details"]
        .as_str()
        .expect("details");
    assert!(
        details.contains("failing could not be reached"),
        "{details}"
    );
}

#[tokio::test]
async fn a_half_closed_client_still_receives_its_answers() {
    let provider = Scripted::new(Vec::new());
    let mut harness = Harness::start(factory(provider, Registry::new), ServerOptions::new());
    let id = harness.take_id();
    harness
        .send(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": { "protocolVersion": 1 },
        }))
        .await;
    harness.write.shutdown().await.expect("half-close");

    let response = harness.read_until(id).await;
    assert_eq!(response["result"]["protocolVersion"], json!(1));
}

#[tokio::test]
async fn a_malformed_line_is_skipped_and_the_next_frame_is_answered() {
    let provider = Scripted::new(Vec::new());
    let mut harness = Harness::start(factory(provider, Registry::new), ServerOptions::new());

    harness.send_raw("this is not json\n\n").await;
    let response = harness
        .call("initialize", json!({ "protocolVersion": 1 }))
        .await;
    assert_eq!(response["result"]["protocolVersion"], json!(1));
}
