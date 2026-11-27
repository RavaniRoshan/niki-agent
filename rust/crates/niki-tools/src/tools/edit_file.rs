use std::fs;

use serde_json::{Value, json};

use crate::{Tool, ToolContext, ToolError, ToolOutput, args, io_error};

/// Replace an exact substring in a file.
pub struct EditFile;

impl Tool for EditFile {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn requires_approval(&self) -> bool {
        true
    }

    fn auto_allows_workspace_write(&self) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Replace an exact substring in a workspace file. A non-unique `old` needs replace_all."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "File path relative to the workspace root."
                },
                "old_string": {
                    "type": "string",
                    "description": "Exact text to replace. Must be unique unless replace_all is true."
                },
                "new_string": {
                    "type": "string",
                    "description": "Replacement text."
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace every occurrence instead of requiring a unique match."
                }
            },
            "required": ["file_path", "old_string", "new_string"],
            "additionalProperties": false
        })
    }

    async fn run(&self, args: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = args::required_str_any(&args, &["file_path", "path"])?;
        let old = args::required_str_any(&args, &["old_string", "old"])?;
        let new = args::required_str_any(&args, &["new_string", "new"])?;
        let replace_all = args::optional_bool(&args, "replace_all")?.unwrap_or(false);

        if old.is_empty() {
            return Err(ToolError::InvalidArguments(
                "`old` must not be empty".to_owned(),
            ));
        }

        let resolved = ctx.resolve(path)?;
        let metadata = fs::metadata(&resolved).map_err(|source| io_error(&resolved, source))?;
        if metadata.len() > crate::MAX_LOADED_FILE_BYTES {
            return Err(ToolError::InvalidArguments(format!(
                "{path} is {} bytes; an edit loads the whole file and the limit is {} bytes",
                metadata.len(),
                crate::MAX_LOADED_FILE_BYTES
            )));
        }
        let original =
            fs::read_to_string(&resolved).map_err(|source| io_error(&resolved, source))?;
        let relative = ctx.relative(&resolved);

        let count = original.matches(old).count();
        match count {
            0 => return Err(ToolError::NoMatch { path: relative }),
            1 => {}
            _ if !replace_all => {
                return Err(ToolError::AmbiguousMatch {
                    path: relative,
                    count,
                });
            }
            _ => {}
        }

        let updated = if replace_all {
            original.replace(old, new)
        } else {
            original.replacen(old, new, 1)
        };
        fs::write(&resolved, updated).map_err(|source| io_error(&resolved, source))?;

        Ok(ToolOutput::new(format!(
            "edited {relative}: {count} replacement{}",
            if count == 1 { "" } else { "s" }
        )))
    }
}
