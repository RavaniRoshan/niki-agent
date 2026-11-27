//! The JSON payload each hook receives on stdin.
//!
//! The shape is a compatible envelope: common `cwd`, `session_id` where the
//! caller has one, `hook_event_name`, and the event's own fields. Tool names
//! are wire names (`shell` is `Bash`, `write_file` is `Write`, …), so a
//! `hooks.json` keeps selecting the same tool.

use std::path::Path;

use serde_json::{Map, Value, json};

use crate::Event;

/// One tool call as the model made it.
#[derive(Debug, Clone, Copy)]
pub struct ToolCall<'a> {
    /// Native tool name, for example `write_file`.
    pub name: &'a str,
    /// Parsed argument object.
    pub input: &'a Value,
    /// The model's id for this call.
    pub id: &'a str,
}

/// A `PreToolUse` invocation.
#[derive(Debug, Clone, Copy)]
pub struct PreToolUse<'a> {
    /// The call about to run.
    pub tool: ToolCall<'a>,
    /// Workspace root, passed to the hook as its `cwd`.
    pub cwd: &'a Path,
    /// Session identity, when the host has one.
    pub session_id: Option<&'a str>,
}

/// A `PostToolUse` invocation.
#[derive(Debug, Clone, Copy)]
pub struct PostToolUse<'a> {
    /// The call that ran.
    pub tool: ToolCall<'a>,
    /// Workspace root, passed to the hook as its `cwd`.
    pub cwd: &'a Path,
    /// Session identity, when the host has one.
    pub session_id: Option<&'a str>,
    /// What the tool returned, as text.
    pub response: &'a Value,
    /// Wall-clock time the call took.
    pub duration_ms: u64,
}

/// A `UserPromptSubmit` invocation.
#[derive(Debug, Clone, Copy)]
pub struct UserPrompt<'a> {
    /// The prompt the person submitted.
    pub prompt: &'a str,
    /// Workspace root, passed to the hook as its `cwd`.
    pub cwd: &'a Path,
    /// Session identity, when the host has one.
    pub session_id: Option<&'a str>,
}

pub(crate) fn pre_tool_use_payload(request: &PreToolUse<'_>) -> Value {
    let mut payload = base(Event::PreToolUse, request.cwd, request.session_id);
    add_tool(&mut payload, request.tool);
    Value::Object(payload)
}

pub(crate) fn post_tool_use_payload(request: &PostToolUse<'_>) -> Value {
    let mut payload = base(Event::PostToolUse, request.cwd, request.session_id);
    add_tool(&mut payload, request.tool);
    payload.insert("tool_response".to_owned(), request.response.clone());
    payload.insert("duration_ms".to_owned(), json!(request.duration_ms));
    Value::Object(payload)
}

pub(crate) fn user_prompt_payload(request: &UserPrompt<'_>) -> Value {
    let mut payload = base(Event::UserPromptSubmit, request.cwd, request.session_id);
    payload.insert("prompt".to_owned(), json!(request.prompt));
    Value::Object(payload)
}

fn base(event: Event, cwd: &Path, session_id: Option<&str>) -> Map<String, Value> {
    let mut payload = Map::new();
    payload.insert("hook_event_name".to_owned(), json!(event.name()));
    payload.insert("cwd".to_owned(), json!(cwd.display().to_string()));
    if let Some(session_id) = session_id {
        payload.insert("session_id".to_owned(), json!(session_id));
    }
    payload
}

fn add_tool(payload: &mut Map<String, Value>, tool: ToolCall<'_>) {
    payload.insert("tool_name".to_owned(), json!(wire_tool_name(tool.name)));
    payload.insert(
        "tool_input".to_owned(),
        wire_tool_input(tool.name, tool.input),
    );
    payload.insert("tool_use_id".to_owned(), json!(tool.id));
}

/// The name used for a matcher and on the wire.
#[must_use]
pub fn wire_tool_name(name: &str) -> &str {
    match name {
        "shell" | "execute" | "bash" => "Bash",
        "write_file" => "Write",
        "edit_file" => "Edit",
        "read_file" => "Read",
        "glob" => "Glob",
        "grep" => "Grep",
        "list_dir" | "ls" => "LS",
        other => other,
    }
}

/// Translate a built-in tool's argument names to the wire vocabulary a hook
/// reads. Unknown tools keep their arguments.
fn wire_tool_input(name: &str, input: &Value) -> Value {
    let Value::Object(input) = input else {
        return input.clone();
    };
    let mut wire = Map::new();
    let mut copy = |from: &str, to: &str| {
        if let Some(value) = input.get(from) {
            wire.insert(to.to_owned(), value.clone());
        }
    };
    match name {
        "shell" | "execute" | "bash" => {
            copy("command", "command");
            copy("timeout", "timeout");
        }
        "write_file" => {
            copy("path", "file_path");
            copy("content", "content");
        }
        "edit_file" => {
            copy("path", "file_path");
            copy("old", "old_string");
            copy("new", "new_string");
            copy("replace_all", "replace_all");
        }
        "read_file" => {
            copy("path", "file_path");
            copy("offset", "offset");
            copy("limit", "limit");
        }
        "glob" => {
            copy("pattern", "pattern");
            copy("path", "path");
        }
        "grep" => {
            copy("pattern", "pattern");
            copy("path", "path");
            copy("glob", "glob");
        }
        "list_dir" | "ls" => copy("path", "path"),
        _ => return Value::Object(input.clone()),
    }
    Value::Object(wire)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{wire_tool_input, wire_tool_name};

    #[test]
    fn built_in_names_map_to_the_wire_vocabulary() {
        assert_eq!(wire_tool_name("shell"), "Bash");
        assert_eq!(wire_tool_name("write_file"), "Write");
        assert_eq!(wire_tool_name("edit_file"), "Edit");
        assert_eq!(wire_tool_name("read_file"), "Read");
        assert_eq!(wire_tool_name("list_dir"), "LS");
        assert_eq!(wire_tool_name("mcp__fixture__write"), "mcp__fixture__write");
    }

    #[test]
    fn built_in_arguments_map_to_the_wire_vocabulary() {
        assert_eq!(
            wire_tool_input("write_file", &json!({"path": "a.txt", "content": "hi"})),
            json!({"file_path": "a.txt", "content": "hi"})
        );
        assert_eq!(
            wire_tool_input("shell", &json!({"command": "ls"})),
            json!({"command": "ls"})
        );
        assert_eq!(
            wire_tool_input("mcp__fixture__write", &json!({"path": "a.txt"})),
            json!({"path": "a.txt"})
        );
    }
}
