use std::fs;

use serde_json::{Value, json};

use crate::{Tool, ToolContext, ToolError, ToolOutput, args, io_error};

/// Write a text file, creating parent directories as needed.
pub struct WriteFile;

impl Tool for WriteFile {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write a text file in the workspace, creating parent directories. Overwrites an existing file."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path relative to the workspace root."
                },
                "content": {
                    "type": "string",
                    "description": "Complete new contents of the file."
                }
            },
            "required": ["path", "content"],
            "additionalProperties": false
        })
    }

    async fn run(&self, args: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = args::required_str(&args, "path")?;
        let content = args::required_str(&args, "content")?;

        let resolved = ctx.resolve(path)?;
        if let Some(parent) = resolved.parent() {
            fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
        }
        fs::write(&resolved, content).map_err(|source| io_error(&resolved, source))?;

        Ok(ToolOutput::new(format!(
            "wrote {} bytes to {}",
            content.len(),
            ctx.relative(&resolved)
        )))
    }
}
