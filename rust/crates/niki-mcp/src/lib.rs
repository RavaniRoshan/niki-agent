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
pub use wire::{DEFAULT_STARTUP_TIMEOUT, DEFAULT_TIMEOUT, PROTOCOL_VERSION, Server};

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use futures_util::future::join_all;

/// Environment variable that overrides the startup budget, in whole seconds.
pub const STARTUP_TIMEOUT_ENV: &str = "NIKI_MCP_STARTUP_TIMEOUT_SECS";

/// Connect every configured server and return the tools it advertised.
///
/// Servers are connected concurrently, so one that never answers its handshake
/// costs its startup budget and no more; the others are not held behind it. A
/// server that fails to connect or list its tools does not hide its siblings:
/// its error is returned beside the tools that loaded, in configuration order.
pub async fn load(servers: &BTreeMap<String, McpServer>) -> (Vec<ToolBridge>, Vec<McpError>) {
    load_with(servers, startup_timeout()).await
}

/// [`load`] with an explicit startup budget, for a host or a test.
///
/// A server that configures `timeout` uses it for both its handshake and every
/// later request, replacing the startup budget passed here; a server without
/// one keeps `startup` for the handshake and [`DEFAULT_TIMEOUT`] per request.
pub async fn load_with(
    servers: &BTreeMap<String, McpServer>,
    startup: Duration,
) -> (Vec<ToolBridge>, Vec<McpError>) {
    let connecting = servers.iter().map(|(name, server)| {
        let name = name.clone();
        let transport = Transport::from(&server.transport);
        let (startup, request) = budgets(server, startup);
        async move { Server::connect_startup_with(&name, transport, startup, request).await }
    });
    let mut tools = Vec::new();
    let mut errors = Vec::new();
    for result in join_all(connecting).await {
        match result {
            Ok(server) => tools.extend(Arc::new(server).bridges()),
            Err(error) => errors.push(error),
        }
    }
    (tools, errors)
}

/// The handshake and per-request budgets for one server.
///
/// A configured `timeout` replaces both. Without one the handshake keeps the
/// global startup budget — `NIKI_MCP_STARTUP_TIMEOUT_SECS` or
/// [`DEFAULT_STARTUP_TIMEOUT`] — and each request keeps [`DEFAULT_TIMEOUT`].
fn budgets(server: &McpServer, startup: Duration) -> (Duration, Duration) {
    server
        .timeout
        .map_or((startup, DEFAULT_TIMEOUT), |timeout| (timeout, timeout))
}

/// The startup budget the environment asks for, or the default.
///
/// A value that is not a positive whole number of seconds is ignored rather
/// than making every server fail immediately.
#[must_use]
pub fn startup_timeout() -> Duration {
    startup_timeout_from(std::env::var(STARTUP_TIMEOUT_ENV).ok().as_deref())
}

fn startup_timeout_from(raw: Option<&str>) -> Duration {
    raw.and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map_or(DEFAULT_STARTUP_TIMEOUT, Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(timeout: Option<Duration>) -> McpServer {
        McpServer {
            transport: niki_config::McpTransport::Http {
                url: "https://example.com/mcp".to_owned(),
                headers: BTreeMap::new(),
            },
            timeout,
        }
    }

    #[test]
    fn the_startup_budget_is_ten_seconds_unless_the_environment_names_another() {
        assert_eq!(startup_timeout_from(None), Duration::from_secs(10));
        assert_eq!(startup_timeout_from(Some("")), Duration::from_secs(10));
        assert_eq!(startup_timeout_from(Some("soon")), Duration::from_secs(10));
        assert_eq!(startup_timeout_from(Some("0")), Duration::from_secs(10));
        assert_eq!(startup_timeout_from(Some(" 3 ")), Duration::from_secs(3));
    }

    #[test]
    fn a_per_server_timeout_replaces_both_budgets() {
        assert_eq!(
            budgets(
                &server(Some(Duration::from_secs(3))),
                Duration::from_secs(10)
            ),
            (Duration::from_secs(3), Duration::from_secs(3))
        );
    }

    #[test]
    fn without_a_per_server_timeout_the_environment_and_request_defaults_apply() {
        let from_env = startup_timeout_from(Some("7"));
        assert_eq!(
            budgets(&server(None), from_env),
            (Duration::from_secs(7), DEFAULT_TIMEOUT)
        );
        assert_eq!(
            budgets(&server(None), startup_timeout_from(None)).0,
            DEFAULT_STARTUP_TIMEOUT
        );
    }
}
