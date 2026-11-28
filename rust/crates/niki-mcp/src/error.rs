use std::time::Duration;

use thiserror::Error;

/// Something one MCP server did wrong, or could not do.
///
/// Every variant names the server and the JSON-RPC method in flight, so a
/// transcript with several servers says which one broke (RULES.md 5).
#[derive(Debug, Error)]
pub enum McpError {
    /// The transport failed while starting, writing, or reading.
    #[error("mcp server `{server}`: {method} failed: {source}")]
    Io {
        /// Server name from configuration.
        server: String,
        /// JSON-RPC method in flight, or `connect` before the handshake.
        method: String,
        /// The underlying I/O failure.
        #[source]
        source: std::io::Error,
    },

    /// The request did not finish inside its budget.
    #[error("mcp server `{server}`: {method} timed out after {timeout:?}")]
    Timeout {
        /// Server name.
        server: String,
        /// JSON-RPC method in flight.
        method: String,
        /// The budget that elapsed.
        timeout: Duration,
    },

    /// The stdio child exited before answering.
    #[error("mcp server `{server}`: process exited during {method} ({status})")]
    Exited {
        /// Server name.
        server: String,
        /// JSON-RPC method in flight.
        method: String,
        /// Exit status, when the process could be reaped.
        status: String,
    },

    /// A response was not the JSON-RPC this client understands.
    #[error("mcp server `{server}`: {method} returned a malformed response: {detail}")]
    Malformed {
        /// Server name.
        server: String,
        /// JSON-RPC method in flight.
        method: String,
        /// What could not be decoded.
        detail: String,
    },

    /// The server answered with a JSON-RPC error object.
    #[error("mcp server `{server}`: {method} returned JSON-RPC error {code}: {message}")]
    Rpc {
        /// Server name.
        server: String,
        /// JSON-RPC method in flight.
        method: String,
        /// Error code from the server.
        code: i64,
        /// Error message from the server.
        message: String,
    },

    /// The HTTP endpoint answered with a non-success status.
    #[error("mcp server `{server}`: {method} returned HTTP {status}")]
    Http {
        /// Server name.
        server: String,
        /// JSON-RPC method in flight.
        method: String,
        /// HTTP status.
        status: u16,
    },

    /// The connection could not be built.
    #[error("mcp server `{server}`: {method} could not start: {detail}")]
    Setup {
        /// Server name.
        server: String,
        /// `connect` while building the transport.
        method: String,
        /// Why the client or transport could not be built.
        detail: String,
    },

    /// An earlier failure left the connection unable to resynchronise, so this
    /// call never reached the server.
    #[error("mcp server `{server}`: {method} cannot run: the connection is closed ({detail})")]
    Closed {
        /// Server name.
        server: String,
        /// JSON-RPC method that was attempted.
        method: String,
        /// The failure that closed the connection.
        detail: String,
    },
}

impl McpError {
    /// The configured server the failure belongs to.
    #[must_use]
    pub fn server(&self) -> &str {
        match self {
            Self::Io { server, .. }
            | Self::Timeout { server, .. }
            | Self::Exited { server, .. }
            | Self::Malformed { server, .. }
            | Self::Rpc { server, .. }
            | Self::Http { server, .. }
            | Self::Setup { server, .. }
            | Self::Closed { server, .. } => server,
        }
    }
}
