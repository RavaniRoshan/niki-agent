//! The MCP client end to end against a real child process.
//!
//! `src/bin/mcp-fixture.rs` is the server: the test binary spawns it with
//! stdio transport, so the handshake, every read timeout and the process exit
//! are exercised for real. No network and no external server (RULES.md 8).

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use niki_mcp::{McpError, PROTOCOL_VERSION, Server, Transport};
use niki_tools::{Registry, ToolContext};
use serde_json::json;

fn fixture() -> Transport {
    Transport::Stdio {
        command: env!("CARGO_BIN_EXE_mcp-fixture").to_owned(),
        args: Vec::new(),
        env: BTreeMap::new(),
        cwd: None,
    }
}

async fn connected() -> Server {
    Server::connect_with("fixture", fixture(), Duration::from_secs(5))
        .await
        .expect("connect to the fixture")
}

#[tokio::test]
async fn initialize_handshake_advertises_the_protocol_version() {
    let server = connected().await;
    assert_eq!(server.name(), "fixture");
    assert_eq!(server.protocol_version(), PROTOCOL_VERSION);
}

#[tokio::test]
async fn tools_list_parses_names_descriptions_and_schemas() {
    let server = connected().await;
    let echo = server
        .tools()
        .iter()
        .find(|tool| tool.name == "echo")
        .expect("echo");
    assert_eq!(echo.description, "Echo the message back.");
    assert_eq!(echo.input_schema["properties"]["message"]["type"], "string");
    assert!(!echo.read_only);

    let peek = server
        .tools()
        .iter()
        .find(|tool| tool.name == "peek")
        .expect("peek");
    assert!(peek.read_only);
}

#[tokio::test]
async fn a_successful_call_returns_the_text_content() {
    let server = connected().await;
    let result = server
        .call_tool("echo", json!({"message": "hello"}))
        .await
        .expect("call");
    assert_eq!(result.text, "hello");
    assert!(!result.is_error);
}

#[tokio::test]
async fn an_is_error_result_is_reported_as_an_error() {
    let server = connected().await;
    let result = server.call_tool("fail", json!({})).await.expect("call");
    assert!(result.is_error);
    assert_eq!(result.text, "boom");
}

#[tokio::test]
async fn a_malformed_response_is_a_decode_error() {
    let server = connected().await;
    let error = server
        .call_tool("malformed", json!({}))
        .await
        .expect_err("malformed");
    assert!(matches!(error, McpError::Malformed { .. }), "{error:?}");
    let text = error.to_string();
    assert!(text.contains("fixture"), "{text}");
    assert!(text.contains("tools/call"), "{text}");
}

#[tokio::test]
async fn a_server_that_exits_mid_call_is_an_error_not_a_hang() {
    let server = connected().await;
    let started = Instant::now();
    let error = server
        .call_tool("exit", json!({}))
        .await
        .expect_err("exited");
    assert!(matches!(error, McpError::Exited { .. }), "{error:?}");
    assert!(error.to_string().contains("fixture"), "{error}");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "took {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn a_server_that_never_answers_times_out() {
    let server = Server::connect_with("fixture", fixture(), Duration::from_millis(200))
        .await
        .expect("connect");
    let error = server
        .call_tool("slow", json!({}))
        .await
        .expect_err("timed out");
    assert!(matches!(error, McpError::Timeout { .. }), "{error:?}");
    assert!(error.to_string().contains("fixture"), "{error}");
}

#[tokio::test]
async fn bridges_register_like_builtins_and_call_through() {
    let server = std::sync::Arc::new(connected().await);
    let bridges = server.bridges();
    assert_eq!(bridges.len(), server.tools().len());

    let mut registry = Registry::new();
    for bridge in bridges {
        registry.register(bridge);
    }

    let echo = registry.get("mcp__fixture__echo").expect("registered");
    assert!(echo.requires_approval(), "echo is not marked read-only");
    assert_eq!(echo.schema()["type"], "object");

    let peek = registry.get("mcp__fixture__peek").expect("registered");
    assert!(!peek.requires_approval(), "a read-only tool is not gated");

    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = ToolContext::new(dir.path()).expect("workspace");
    let output = echo
        .run(json!({"message": "hi"}), &ctx)
        .await
        .expect("run the bridge");
    assert_eq!(output.text, "hi");

    let specs = registry.specs();
    let names: Vec<&str> = specs
        .as_array()
        .expect("specs")
        .iter()
        .filter_map(|spec| spec.get("name").and_then(serde_json::Value::as_str))
        .collect();
    assert!(names.contains(&"mcp__fixture__echo"), "{names:?}");
}

#[tokio::test]
async fn a_bridge_surfaces_an_is_error_result_as_a_tool_error() {
    let server = std::sync::Arc::new(connected().await);
    let mut registry = Registry::new();
    for bridge in server.bridges() {
        registry.register(bridge);
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = ToolContext::new(dir.path()).expect("workspace");
    let fail = registry.get("mcp__fixture__fail").expect("registered");
    let error = fail.run(json!({}), &ctx).await.expect_err("tool error");
    assert!(error.to_string().contains("boom"), "{error}");
}
