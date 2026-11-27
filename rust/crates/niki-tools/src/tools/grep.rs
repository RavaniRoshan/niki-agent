use std::fs;

use globset::{Glob as GlobPattern, GlobMatcher};
use regex::Regex;
use serde_json::{Value, json};

use crate::output::{MAX_OUTPUT_BYTES, MAX_OUTPUT_LINES};
use crate::{Tool, ToolContext, ToolError, ToolOutput, args, io_error, walk};

/// Search the workspace with a regular expression.
pub struct Grep;

impl Tool for Grep {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search workspace files with a regular expression. Returns path:line:text."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regular expression matched against each line."
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search, relative to the workspace root. Defaults to the root."
                },
                "glob": {
                    "type": "string",
                    "description": "Glob filter applied to relative file paths, for example `**/*.rs`."
                }
            },
            "required": ["pattern"],
            "additionalProperties": false
        })
    }

    async fn run(&self, args: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let pattern = args::required_str(&args, "pattern")?;
        let requested = args::optional_str(&args, "path")?.unwrap_or(".");
        let glob_pattern = args::optional_str(&args, "glob")?;

        let regex = Regex::new(pattern).map_err(|error| ToolError::Regex {
            pattern: pattern.to_owned(),
            message: error.to_string(),
        })?;
        let matcher = match glob_pattern {
            Some(glob) => Some(compile_glob(glob)?),
            None => None,
        };

        let resolved = ctx.resolve(requested)?;
        let metadata = fs::metadata(&resolved).map_err(|source| io_error(&resolved, source))?;
        let files = if metadata.is_dir() {
            walk::files(&resolved)?
        } else if metadata.is_file() {
            vec![resolved]
        } else {
            return Err(ToolError::InvalidArguments(format!(
                "`{requested}` is neither a file nor a directory"
            )));
        };

        let mut lines = Vec::new();
        let mut bytes = 0usize;

        'files: for file in files {
            let relative = ctx.relative(&file);
            if matcher
                .as_ref()
                .is_some_and(|matcher| !matcher.is_match(&relative))
            {
                continue;
            }
            // A file that cannot be read must not abort a workspace-wide search.
            let Ok(metadata) = fs::metadata(&file) else {
                continue;
            };
            if metadata.len() > crate::MAX_LOADED_FILE_BYTES {
                continue;
            }
            let Ok(contents) = fs::read(&file) else {
                continue;
            };
            // NUL marks a binary file whose lines would only be noise to the model.
            if contents.contains(&0) {
                continue;
            }
            let text = String::from_utf8_lossy(&contents);
            for (index, line) in text.lines().enumerate() {
                let line = line.strip_suffix('\r').unwrap_or(line);
                if !regex.is_match(line) {
                    continue;
                }
                let entry = format!("{relative}:{}:{line}", index + 1);
                bytes += entry.len() + 1;
                lines.push(entry);
                if bytes > MAX_OUTPUT_BYTES || lines.len() > MAX_OUTPUT_LINES {
                    break 'files;
                }
            }
        }

        Ok(ToolOutput::new(lines.join("\n")))
    }
}

fn compile_glob(pattern: &str) -> Result<GlobMatcher, ToolError> {
    GlobPattern::new(pattern)
        .map(|glob| glob.compile_matcher())
        .map_err(|error| ToolError::Glob {
            pattern: pattern.to_owned(),
            message: error.to_string(),
        })
}
