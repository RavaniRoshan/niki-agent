//! MCP servers for the Rust client.
//!
//! A [`Server`] speaks JSON-RPC 2.0 to one configured server: `initialize`,
//! `tools/list`, then `tools/call`. Two transports exist, matching the shape
//! `[mcp_servers.<name>]` accepts:
//!
//! - stdio: the configured command is spawned and messages are line-delimited
//!   JSON on its stdin/stdout.
//! - HTTP: one POST per message. A JSON body is read directly; an SSE body is
//!   parsed for the response with the matching id. A legacy `sse` transport
//!   (a long-lived GET stream) is not implemented and is rejected at config
//!   load, so a server using it fails with a message rather than a hang.
//!
//! Every read is bounded by a timeout and a child that exits is reported as
//! [`McpError::Exited`]; neither can hang the turn (RULES.md 4).
//!
//! [`ToolBridge`] adapts each discovered tool to [`niki_tools::Tool`] under the
//! name `mcp__<server>__<tool>`, so the agent loop dispatches it exactly like a
//! built-in. A server's `readOnlyHint` decides whether the bridge asks the
//! approval policy first: a tool that does not make that claim is gated.

#![forbid(unsafe_code)]

mod error;
mod tool;
mod transport;
mod wire;

pub use error::McpError;
pub use niki_config::McpServer;
pub use tool::{CallResult, McpTool, ToolBridge, tool_name};
pub use transport::Transport;
pub use wire::{DEFAULT_TIMEOUT, PROTOCOL_VERSION, Server};

use std::collections::BTreeMap;
use std::sync::Arc;

/// Connect every configured server and return the tools it advertised.
///
/// A server that fails to connect or list its tools does not hide its
/// siblings: its error is returned beside the tools that loaded, in
/// configuration order.
pub async fn load(servers: &BTreeMap<String, McpServer>) -> (Vec<ToolBridge>, Vec<McpError>) {
    let mut tools = Vec::new();
    let mut errors = Vec::new();
    for (name, server) in servers {
        match Server::connect(name, Transport::from(&server.transport)).await {
            Ok(server) => {
                let server = Arc::new(server);
                tools.extend(server.bridges());
            }
            Err(error) => errors.push(error),
        }
    }
    (tools, errors)
}
