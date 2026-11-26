use std::fs;

use serde_json::{Value, json};

use crate::{Tool, ToolContext, ToolError, ToolOutput, args, io_error};

/// Replace an exact substring in a file.
pub struct EditFile;

impl Tool for EditFile {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Replace an exact substring in a workspace file. A non-unique `old` needs replace_all."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path relative to the workspace root."
                },
                "old": {
                    "type": "string",
                    "description": "Exact text to replace. Must be unique unless replace_all is true."
                },
                "new": {
                    "type": "string",
                    "description": "Replacement text."
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace every occurrence instead of requiring a unique match."
                }
            },
            "required": ["path", "old", "new"],
            "additionalProperties": false
        })
    }

    async fn run(&self, args: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = args::required_str(&args, "path")?;
        let old = args::required_str(&args, "old")?;
        let new = args::required_str(&args, "new")?;
        let replace_all = args::optional_bool(&args, "replace_all")?.unwrap_or(false);

        if old.is_empty() {
            return Err(ToolError::InvalidArguments(
                "`old` must not be empty".to_owned(),
            ));
        }

        let resolved = ctx.resolve(path)?;
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
