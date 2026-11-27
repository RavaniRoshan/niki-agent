use std::fs;

use serde_json::{Value, json};

use crate::{Tool, ToolContext, ToolError, ToolOutput, args, io_error};

/// List one directory, directories marked with a trailing slash.
pub struct ListDir;

impl Tool for ListDir {
    fn name(&self) -> &str {
        "list_dir"
    }

    fn description(&self) -> &str {
        "List the entries of a workspace directory. Directories end with a slash."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path relative to the workspace root."
                }
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    async fn run(&self, args: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = args::required_str_any(&args, &["file_path", "path"])?;
        let resolved = ctx.resolve(path)?;

        let mut entries = Vec::new();
        for entry in fs::read_dir(&resolved).map_err(|source| io_error(&resolved, source))? {
            let entry = entry.map_err(|source| io_error(&resolved, source))?;
            let file_type = entry
                .file_type()
                .map_err(|source| io_error(&entry.path(), source))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if file_type.is_dir() {
                entries.push(format!("{name}/"));
            } else {
                entries.push(name);
            }
        }
        entries.sort();

        Ok(ToolOutput::new(entries.join("\n")))
    }
}
