use globset::Glob as GlobPattern;
use serde_json::{Value, json};

use crate::output::{MAX_OUTPUT_BYTES, MAX_OUTPUT_LINES};
use crate::{Tool, ToolContext, ToolError, ToolOutput, args, walk};

/// Find files whose path relative to the root matches a glob.
pub struct Glob;

impl Tool for Glob {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find workspace files by glob pattern, relative to the workspace root and sorted."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern matched against paths relative to the workspace root, for example `**/*.rs`."
                }
            },
            "required": ["pattern"],
            "additionalProperties": false
        })
    }

    async fn run(&self, args: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let pattern = args::required_str(&args, "pattern")?;
        let matcher = GlobPattern::new(pattern)
            .map(|glob| glob.compile_matcher())
            .map_err(|error| ToolError::Glob {
                pattern: pattern.to_owned(),
                message: error.to_string(),
            })?;

        let root = ctx.resolve(".")?;
        let files = walk::files(&root)?;

        let mut matches = Vec::new();
        let mut bytes = 0usize;
        for file in files {
            let relative = ctx.relative(&file);
            if !matcher.is_match(&relative) {
                continue;
            }
            bytes += relative.len() + 1;
            matches.push(relative);
            if bytes > MAX_OUTPUT_BYTES || matches.len() > MAX_OUTPUT_LINES {
                break;
            }
        }

        Ok(ToolOutput::new(matches.join("\n")))
    }
}
