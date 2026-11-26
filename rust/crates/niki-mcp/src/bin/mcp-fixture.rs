//! A tiny stdio MCP server used by `niki-mcp`'s integration tests.
//!
//! It is a test fixture, not a product binary: it answers the handshake,
//! advertises one tool per protocol behaviour the tests cover, and reports
//! failures the way a real server might. `tools/call` chooses the behaviour
//! from the tool name:
//!
//! - `echo` — returns the `message` argument as text.
//! - `peek` — a read-only tool (`readOnlyHint`).
//! - `fail` — returns `isError: true` with `boom`.
//! - `malformed` — writes a line that is not JSON.
//! - `exit` — exits without answering.
//! - `slow` — sleeps two seconds before answering.

use std::io::{BufRead, Write};

use serde_json::{Value, json};

fn main() {
    if std::env::var_os("MCP_FIXTURE_SILENT").is_some() {
        // A server that accepts the process and then never answers the
        // handshake, which is what the startup budget is measured against.
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
    }
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            return;
        };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(request) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        handle(&request, &mut stdout);
    }
}

fn handle(request: &Value, stdout: &mut impl Write) {
    let Some(id) = request.get("id").filter(|id| !id.is_null()).cloned() else {
        // A notification needs no reply.
        return;
    };
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let reply = match method {
        "initialize" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2025-06-18",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "niki-mcp-fixture", "version": "0.0.0"},
            },
        }),
        "ping" => json!({"jsonrpc": "2.0", "id": id, "result": {}}),
        "tools/list" => json!({"jsonrpc": "2.0", "id": id, "result": {"tools": tools()}}),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or(Value::Null);
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            match name {
                "exit" => std::process::exit(7),
                "malformed" => {
                    let _ = writeln!(stdout, "this is not JSON");
                    let _ = stdout.flush();
                    return;
                }
                "slow" => {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    call_reply(id, "slow", false)
                }
                "fail" => call_reply(id, "boom", true),
                _ => {
                    let message = params
                        .pointer("/arguments/message")
                        .and_then(Value::as_str)
                        .unwrap_or("pong");
                    call_reply(id, message, false)
                }
            }
        }
        other => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32601, "message": format!("method not found: {other}")},
        }),
    };
    let _ = writeln!(stdout, "{reply}");
    let _ = stdout.flush();
}

fn tools() -> Vec<Value> {
    vec![
        json!({
            "name": "echo",
            "description": "Echo the message back.",
            "inputSchema": {
                "type": "object",
                "properties": {"message": {"type": "string"}},
                "required": ["message"],
            },
        }),
        json!({
            "name": "peek",
            "description": "Read without changing anything.",
            "inputSchema": {"type": "object"},
            "annotations": {"readOnlyHint": true},
        }),
        json!({
            "name": "fail",
            "description": "Always fails.",
            "inputSchema": {"type": "object"},
            "annotations": {"readOnlyHint": false},
        }),
        json!({
            "name": "malformed",
            "description": "Answers with a line that is not JSON.",
            "inputSchema": {"type": "object"},
        }),
        json!({
            "name": "exit",
            "description": "Exits without answering.",
            "inputSchema": {"type": "object"},
        }),
        json!({
            "name": "slow",
            "description": "Answers after a delay.",
            "inputSchema": {"type": "object"},
        }),
    ]
}

fn call_reply(id: Value, text: &str, is_error: bool) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{"type": "text", "text": text}],
            "isError": is_error,
        },
    })
}
