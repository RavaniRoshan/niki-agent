use std::io;

use thiserror::Error;

/// A tool failure the model or the caller can act on.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("{0}")]
    InvalidArguments(String),

    #[error("path escapes the workspace root {root}: {path}")]
    OutsideRoot { root: String, path: String },

    #[error("{path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("`old` does not occur in {path}")]
    NoMatch { path: String },

    #[error("`old` occurs {count} times in {path}; pass replace_all to replace every occurrence")]
    AmbiguousMatch { path: String, count: usize },

    #[error("invalid regular expression `{pattern}`: {message}")]
    Regex { pattern: String, message: String },

    #[error("invalid glob pattern `{pattern}`: {message}")]
    Glob { pattern: String, message: String },

    #[error("`{command}` timed out after {timeout:?}")]
    Timeout {
        command: String,
        timeout: std::time::Duration,
    },

    /// A tool backed by an external service — an MCP server — failed, or the
    /// server reported the tool's own `isError` result.
    #[error("{0}")]
    Mcp(String),
}
