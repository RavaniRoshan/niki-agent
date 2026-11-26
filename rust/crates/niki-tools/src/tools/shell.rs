use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::process::Command;

use crate::{Tool, ToolContext, ToolError, ToolOutput, args, io_error};

/// Wall-clock limit for one shell call.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Run a command with `/bin/sh -c`, reporting the exit code and both streams.
pub struct Shell {
    timeout: Duration,
}

impl Shell {
    /// A shell tool with the default 120 s timeout.
    #[must_use]
    pub fn new() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// A shell tool with a different timeout, for tests and callers that set
    /// their own budget.
    #[must_use]
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for Shell {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Run a shell command in the workspace with /bin/sh -c. Returns the exit code, stdout and stderr."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Command line passed to /bin/sh -c."
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory relative to the workspace root. Defaults to the root."
                }
            },
            "required": ["command"],
            "additionalProperties": false
        })
    }

    async fn run(&self, args: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let command = args::required_str(&args, "command")?;
        let cwd = ctx.resolve(args::optional_str(&args, "cwd")?.unwrap_or("."))?;

        let child = Command::new("/bin/sh")
            .arg("-c")
            .arg(command)
            .current_dir(&cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|source| io_error(&cwd, source))?;

        // `wait_with_output` takes ownership, so dropping the timed-out future
        // drops the child and `kill_on_drop` terminates it.
        let output = match tokio::time::timeout(self.timeout, child.wait_with_output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(source)) => return Err(io_error(&cwd, source)),
            Err(_) => {
                return Err(ToolError::Timeout {
                    command: command.to_owned(),
                    timeout: self.timeout,
                });
            }
        };

        let status = output
            .status
            .code()
            .map_or_else(|| "signal".to_owned(), |code| code.to_string());
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut text = format!("exit_code: {status}\n--- stdout ---\n{stdout}");
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str("--- stderr ---\n");
        text.push_str(&stderr);
        if !text.ends_with('\n') {
            text.push('\n');
        }

        Ok(ToolOutput::new(text))
    }
}
