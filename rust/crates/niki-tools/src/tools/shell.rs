use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

use crate::{MAX_OUTPUT_BYTES, Tool, ToolContext, ToolError, ToolOutput, args, io_error};

/// Wall-clock limit for one shell call.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Bytes kept from each stream before the rest is drained and noted.
///
/// A command can print without limit — `yes`, `cat /dev/zero`, a test runner
/// in a loop — so the reader keeps a bounded prefix and discards the rest
/// instead of buffering the whole stream in memory.
const CAPTURE_LIMIT: usize = MAX_OUTPUT_BYTES;

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

        let mut child = Command::new("/bin/sh")
            .arg("-c")
            .arg(command)
            .current_dir(&cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|source| io_error(&cwd, source))?;

        let stdout_pipe = child.stdout.take().expect("stdout is piped");
        let stderr_pipe = child.stderr.take().expect("stderr is piped");
        let stdout_reader = tokio::spawn(read_capped(stdout_pipe));
        let stderr_reader = tokio::spawn(read_capped(stderr_pipe));

        let status = match tokio::time::timeout(self.timeout, child.wait()).await {
            Ok(Ok(status)) => status,
            Ok(Err(source)) => {
                stdout_reader.abort();
                stderr_reader.abort();
                return Err(io_error(&cwd, source));
            }
            Err(_) => {
                stdout_reader.abort();
                stderr_reader.abort();
                let _ = child.start_kill();
                return Err(ToolError::Timeout {
                    command: command.to_owned(),
                    timeout: self.timeout,
                });
            }
        };

        let (stdout, stdout_cut) = stdout_reader.await.unwrap_or_else(|_| (Vec::new(), false));
        let (stderr, stderr_cut) = stderr_reader.await.unwrap_or_else(|_| (Vec::new(), false));

        let status = status
            .code()
            .map_or_else(|| "signal".to_owned(), |code| code.to_string());
        let mut text = format!(
            "exit_code: {status}\n{}\n",
            stream_header("stdout", stdout_cut)
        );
        push_stream(&mut text, &String::from_utf8_lossy(&stdout));
        text.push_str(&format!("{}\n", stream_header("stderr", stderr_cut)));
        push_stream(&mut text, &String::from_utf8_lossy(&stderr));

        Ok(ToolOutput::new(text))
    }
}

/// Read `reader` to the end, keeping at most [`CAPTURE_LIMIT`] bytes.
async fn read_capped(mut reader: impl AsyncRead + Unpin) -> (Vec<u8>, bool) {
    let mut kept = Vec::new();
    let mut cut = false;
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                let room = CAPTURE_LIMIT.saturating_sub(kept.len());
                let take = room.min(read);
                kept.extend_from_slice(&buffer[..take]);
                if take < read {
                    cut = true;
                }
            }
        }
    }
    (kept, cut)
}

/// The line that opens a captured stream, naming the cap when it was hit.
fn stream_header(name: &str, cut: bool) -> String {
    if cut {
        format!("--- {name} (truncated, first {CAPTURE_LIMIT} bytes) ---")
    } else {
        format!("--- {name} ---")
    }
}

fn push_stream(text: &mut String, stream: &str) {
    text.push_str(stream);
    if !text.ends_with('\n') {
        text.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_stream_under_the_limit_is_kept_whole() {
        let data = vec![b'a'; 100];
        let (kept, cut) = read_capped(&data[..]).await;
        assert_eq!(kept, data);
        assert!(!cut);
    }

    #[tokio::test]
    async fn a_flood_is_cut_at_the_limit_and_never_buffered_whole() {
        let data = vec![b'a'; CAPTURE_LIMIT * 32];
        let (kept, cut) = read_capped(&data[..]).await;
        assert_eq!(kept.len(), CAPTURE_LIMIT);
        assert!(cut);
    }

    #[tokio::test]
    async fn an_empty_stream_is_kept_and_not_marked() {
        let (kept, cut) = read_capped(&b""[..]).await;
        assert!(kept.is_empty());
        assert!(!cut);
    }
}
