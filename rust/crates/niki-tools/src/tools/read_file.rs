use std::fs::File;
use std::io::{BufRead, BufReader};

use serde_json::{Value, json};

use crate::output::{MAX_OUTPUT_BYTES, MAX_OUTPUT_LINES};
use crate::{Tool, ToolContext, ToolError, ToolOutput, args, io_error};

/// Read a text file, one output line per file line.
pub struct ReadFile;

impl Tool for ReadFile {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read a text file from the workspace. Use offset and limit to page a large file."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path relative to the workspace root."
                },
                "offset": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "1-based line to start at. Defaults to 1."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Maximum number of lines to return."
                }
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    async fn run(&self, args: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = args::required_str_any(&args, &["file_path", "path"])?;
        let offset = u64::from(args::optional_u32(&args, "offset")?.unwrap_or(1).max(1));
        let limit =
            args::optional_u32(&args, "limit")?.and_then(|limit| usize::try_from(limit).ok());

        let resolved = ctx.resolve(path)?;
        let file = File::open(&resolved).map_err(|source| io_error(&resolved, source))?;
        let mut reader = BufReader::new(file);

        let mut buffer = Vec::new();
        let mut line_number = 0u64;
        let mut emitted = 0usize;
        let mut text = String::new();

        loop {
            buffer.clear();
            let read = reader
                .read_until(b'\n', &mut buffer)
                .map_err(|source| io_error(&resolved, source))?;
            if read == 0 {
                break;
            }
            line_number += 1;
            if line_number < offset {
                continue;
            }
            if limit.is_some_and(|limit| emitted >= limit) {
                break;
            }
            text.push_str(&String::from_utf8_lossy(&buffer));
            emitted += 1;
            if text.len() > MAX_OUTPUT_BYTES || (limit.is_none() && emitted > MAX_OUTPUT_LINES) {
                break;
            }
        }

        Ok(ToolOutput::new(text))
    }
}
