use std::collections::BTreeMap;
use std::path::PathBuf;

use niki_config::McpTransport;

/// How to reach one MCP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    /// A child process speaking line-delimited JSON-RPC on stdin/stdout.
    Stdio {
        /// Executable to run.
        command: String,
        /// Arguments passed to the executable.
        args: Vec<String>,
        /// Extra environment variables; the parent's environment is inherited.
        env: BTreeMap<String, String>,
        /// Working directory for the child.
        cwd: Option<PathBuf>,
    },
    /// Streamable HTTP: one POST per message, answered with JSON or SSE.
    Http {
        /// Endpoint URL.
        url: String,
        /// Headers sent with every request.
        headers: BTreeMap<String, String>,
    },
}

impl Transport {
    /// A stdio transport for `command` with no extra arguments or environment.
    #[must_use]
    pub fn stdio(command: impl Into<String>) -> Self {
        Self::Stdio {
            command: command.into(),
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: None,
        }
    }

    /// An HTTP transport for `url` with no extra headers.
    #[must_use]
    pub fn http(url: impl Into<String>) -> Self {
        Self::Http {
            url: url.into(),
            headers: BTreeMap::new(),
        }
    }
}

impl From<&McpTransport> for Transport {
    fn from(transport: &McpTransport) -> Self {
        match transport {
            McpTransport::Stdio {
                command,
                args,
                env,
                cwd,
            } => Self::Stdio {
                command: command.clone(),
                args: args.clone(),
                env: env.clone(),
                cwd: cwd.clone(),
            },
            McpTransport::Http { url, headers } => Self::Http {
                url: url.clone(),
                headers: headers.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_configured_stdio_entry_becomes_a_stdio_transport() {
        let mut env = BTreeMap::new();
        env.insert("LOG_LEVEL".to_owned(), "debug".to_owned());
        let configured = McpTransport::Stdio {
            command: "npx".to_owned(),
            args: vec!["server".to_owned()],
            env,
            cwd: Some(PathBuf::from("/tmp/work")),
        };
        let Transport::Stdio {
            command,
            args,
            env,
            cwd,
        } = Transport::from(&configured)
        else {
            panic!("expected stdio");
        };
        assert_eq!(command, "npx");
        assert_eq!(args, vec!["server".to_owned()]);
        assert_eq!(env.get("LOG_LEVEL").map(String::as_str), Some("debug"));
        assert_eq!(cwd, Some(PathBuf::from("/tmp/work")));
    }

    #[test]
    fn a_configured_http_entry_becomes_an_http_transport() {
        let mut headers = BTreeMap::new();
        headers.insert("Authorization".to_owned(), "Bearer token".to_owned());
        let configured = McpTransport::Http {
            url: "https://example.com/mcp".to_owned(),
            headers,
        };
        let Transport::Http { url, headers } = Transport::from(&configured) else {
            panic!("expected http");
        };
        assert_eq!(url, "https://example.com/mcp");
        assert_eq!(
            headers.get("Authorization").map(String::as_str),
            Some("Bearer token")
        );
    }
}
