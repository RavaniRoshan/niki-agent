//! The Agent Client Protocol wire shapes this server emits and accepts.
//!
//! Field names are the protocol schema's (`sessionId`, `stopReason`,
//! `toolCallId`, `rawInput`, `sessionUpdate`), copied from the published
//! schema rather than invented. Everything the schema marks optional is left
//! out when unset, matching the reference server's `exclude_none` dump.

use niki_policy::Mode;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::rpc::RpcError;

/// One content block of a prompt.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ContentBlock {
    Text {
        text: String,
    },
    Image {
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    Audio {
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    ResourceLink {
        name: String,
        uri: String,
        #[serde(default)]
        description: Option<String>,
        #[serde(default, rename = "mimeType")]
        mime_type: Option<String>,
    },
    Resource {
        resource: EmbeddedResource,
    },
}

/// The resource payload of an embedded resource block.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum EmbeddedResource {
    Text {
        text: String,
        #[serde(default, rename = "mimeType")]
        mime_type: Option<String>,
    },
    Blob {
        blob: String,
        #[serde(default, rename = "mimeType")]
        mime_type: Option<String>,
    },
}

/// Flatten a prompt into the text the turn loop takes.
///
/// Text blocks and resource metadata become text; the reference server hands
/// images to the model as `image_url` parts, which this client's message model
/// cannot carry, so they are refused rather than silently dropped.
pub(crate) fn prompt_text(blocks: &[ContentBlock]) -> Result<String, RpcError> {
    let mut parts = Vec::with_capacity(blocks.len());
    for block in blocks {
        match block {
            ContentBlock::Text { text } => parts.push(text.clone()),
            ContentBlock::ResourceLink {
                name,
                uri,
                description,
                mime_type,
            } => parts.push(resource_placeholder(
                name,
                uri,
                description.as_deref(),
                mime_type.as_deref(),
            )),
            ContentBlock::Resource {
                resource: EmbeddedResource::Text { text, mime_type },
            } => parts.push(format!(
                "[Embedded {} resource: {text}]",
                mime_type.as_deref().unwrap_or("text")
            )),
            ContentBlock::Resource {
                resource: EmbeddedResource::Blob { blob, mime_type },
            } => parts.push(format!(
                "[Embedded resource: data:{};base64,{blob}]",
                mime_type.as_deref().unwrap_or("application/octet-stream")
            )),
            ContentBlock::Image { mime_type } => {
                return Err(RpcError::invalid_params_field(
                    "prompt",
                    format!(
                        "image content ({mime_type}) is not supported; \
                         this server advertises text prompts only"
                    ),
                ));
            }
            ContentBlock::Audio { mime_type } => {
                return Err(RpcError::invalid_params_field(
                    "prompt",
                    format!("audio content ({mime_type}) is not supported"),
                ));
            }
        }
    }
    Ok(parts.join("\n"))
}

fn resource_placeholder(
    name: &str,
    uri: &str,
    description: Option<&str>,
    mime_type: Option<&str>,
) -> String {
    let mut text = format!("[Resource: {name}\nURI: {uri}");
    if let Some(description) = description {
        text.push_str(&format!("\nDescription: {description}"));
    }
    if let Some(mime_type) = mime_type {
        text.push_str(&format!("\nMIME type: {mime_type}"));
    }
    text.push(']');
    text
}

/// The lowercase id of one approval mode.
pub(crate) fn mode_id(mode: Mode) -> &'static str {
    match mode {
        Mode::Manual => "manual",
        Mode::Auto => "auto",
        Mode::Yolo => "yolo",
    }
}

pub(crate) fn mode_from_id(id: &str) -> Option<Mode> {
    match id {
        "manual" => Some(Mode::Manual),
        "auto" => Some(Mode::Auto),
        "yolo" => Some(Mode::Yolo),
        _ => None,
    }
}

fn mode_words(mode: Mode) -> (&'static str, &'static str) {
    match mode {
        Mode::Manual => ("Manual", "Ask before a command or gated tool runs"),
        Mode::Auto => (
            "Auto",
            "Run allow-listed commands without asking; ask for the rest",
        ),
        Mode::Yolo => ("YOLO", "Run everything the policy permits without asking"),
    }
}

/// `modes` in a session response: the current mode and every selectable one.
pub(crate) fn modes_state(mode: Mode) -> Value {
    let available: Vec<Value> = [Mode::Manual, Mode::Auto, Mode::Yolo]
        .into_iter()
        .map(|mode| {
            let (name, description) = mode_words(mode);
            json!({
                "id": mode_id(mode),
                "name": name,
                "description": description,
            })
        })
        .collect();
    json!({
        "currentModeId": mode_id(mode),
        "availableModes": available,
    })
}

/// `configOptions` in a session response: the mode selector.
pub(crate) fn config_options(mode: Mode) -> Value {
    let options: Vec<Value> = [Mode::Manual, Mode::Auto, Mode::Yolo]
        .into_iter()
        .map(|mode| {
            let (name, description) = mode_words(mode);
            json!({
                "value": mode_id(mode),
                "name": name,
                "description": description,
            })
        })
        .collect();
    json!([{
        "id": "mode",
        "name": "Session Mode",
        "description": "Controls how the agent requests permission",
        "category": "mode",
        "type": "select",
        "currentValue": mode_id(mode),
        "options": options,
    }])
}

/// How one tool call is presented in a `tool_call` update.
pub(crate) struct Presentation {
    pub(crate) title: String,
    pub(crate) kind: &'static str,
    /// Inline content, currently only the diff an edit will apply.
    pub(crate) content: Option<Value>,
}

/// The title, kind and inline content of one call, by the tool's own names.
pub(crate) fn present(name: &str, arguments: &Value) -> Presentation {
    let field = |key: &str| arguments.get(key).and_then(Value::as_str).unwrap_or("");
    let named = |label: &str, value: &str| {
        if value.is_empty() {
            name.to_owned()
        } else {
            format!("{label} `{value}`")
        }
    };
    match name {
        "read_file" => Presentation {
            title: named("Read", field("path")),
            kind: "read",
            content: None,
        },
        "write_file" => Presentation {
            title: named("Write", field("path")),
            kind: "edit",
            content: None,
        },
        "edit_file" => {
            let path = field("path");
            let old = field("old");
            let new = field("new");
            let content = (!path.is_empty() && !old.is_empty() && !new.is_empty()).then(|| {
                json!([{
                    "type": "diff",
                    "path": path,
                    "oldText": old,
                    "newText": new,
                }])
            });
            Presentation {
                title: named("Edit", path),
                kind: "edit",
                content,
            }
        }
        "list_dir" => Presentation {
            title: named("List", field("path")),
            kind: "search",
            content: None,
        },
        "glob" => Presentation {
            title: named("Glob", field("pattern")),
            kind: "search",
            content: None,
        },
        "grep" => Presentation {
            title: named("Grep", field("pattern")),
            kind: "search",
            content: None,
        },
        "shell" => Presentation {
            title: truncate_for_display(field("command")),
            kind: "execute",
            content: None,
        },
        _ => Presentation {
            title: name.to_owned(),
            kind: "other",
            content: None,
        },
    }
}

/// Longest shell command shown in a tool-call title, before the ellipsis.
const MAX_TITLE_COMMAND: usize = 120;

fn truncate_for_display(command: &str) -> String {
    if command.chars().count() >= MAX_TITLE_COMMAND {
        command.chars().take(MAX_TITLE_COMMAND).collect::<String>() + "..."
    } else {
        command.to_owned()
    }
}

/// Tools whose arguments carry a plan the client renders as `plan` entries.
pub(crate) const TODO_TOOLS: [&str; 2] = ["write_todos", "todo_write"];

/// A `plan` update from a todo tool's arguments, or `None` when there are none.
pub(crate) fn plan_update(arguments: &Value) -> Option<Value> {
    let todos = arguments.get("todos")?.as_array()?;
    let entries: Vec<Value> = todos
        .iter()
        .map(|todo| {
            let content = todo
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let status = match todo.get("status").and_then(Value::as_str) {
                Some("in_progress") => "in_progress",
                Some("completed") => "completed",
                _ => "pending",
            };
            json!({
                "content": content,
                "status": status,
                "priority": "medium",
            })
        })
        .collect();
    Some(json!({ "sessionUpdate": "plan", "entries": entries }))
}

/// Parse a tool call's argument string, treating anything unparsable as none.
pub(crate) fn arguments_value(arguments: &str) -> Value {
    serde_json::from_str(arguments).unwrap_or(Value::Null)
}

/// The `rawInput` member, present only when the arguments are an object.
pub(crate) fn raw_input(arguments: &Value) -> Option<Value> {
    arguments.is_object().then(|| arguments.clone())
}

pub(crate) fn text_block(text: &str) -> Value {
    json!({ "type": "text", "text": text })
}

/// A `user_message_chunk`, `agent_message_chunk` or `agent_thought_chunk`.
pub(crate) fn message_chunk(kind: &str, text: &str, message_id: Option<&str>) -> Value {
    let mut update = json!({
        "sessionUpdate": kind,
        "content": text_block(text),
    });
    if let Some(message_id) = message_id {
        update["messageId"] = json!(message_id);
    }
    update
}

pub(crate) fn tool_call_start(
    tool_call_id: &str,
    presentation: &Presentation,
    raw_input: Option<Value>,
) -> Value {
    let mut update = json!({
        "sessionUpdate": "tool_call",
        "toolCallId": tool_call_id,
        "title": presentation.title,
        "kind": presentation.kind,
        "status": "pending",
    });
    if let Some(content) = &presentation.content {
        update["content"] = content.clone();
    }
    if let Some(raw_input) = raw_input {
        update["rawInput"] = raw_input;
    }
    update
}

pub(crate) fn tool_call_update(tool_call_id: &str, status: &str, content: Option<&str>) -> Value {
    let mut update = json!({
        "sessionUpdate": "tool_call_update",
        "toolCallId": tool_call_id,
        "status": status,
    });
    if let Some(content) = content {
        update["content"] = json!([{
            "type": "content",
            "content": text_block(content),
        }]);
    }
    update
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_block(value: Value) -> ContentBlock {
        serde_json::from_value(value).expect("block")
    }

    #[test]
    fn text_and_resource_blocks_become_prompt_text() {
        let blocks = vec![
            parse_block(json!({ "type": "text", "text": "look at this" })),
            parse_block(json!({
                "type": "resource_link",
                "name": "main.rs",
                "uri": "file:///work/src/main.rs",
                "description": "entry point",
                "mimeType": "text/rust",
            })),
            parse_block(json!({
                "type": "resource",
                "resource": { "uri": "file:///a", "text": "contents", "mimeType": "text/plain" },
            })),
        ];
        let text = prompt_text(&blocks).expect("text");
        assert_eq!(
            text,
            "look at this\n[Resource: main.rs\nURI: file:///work/src/main.rs\nDescription: entry point\nMIME type: text/rust]\n[Embedded text/plain resource: contents]"
        );
    }

    #[test]
    fn an_image_block_is_refused_with_the_reason() {
        let block = parse_block(json!({
            "type": "image",
            "data": "AAAA",
            "mimeType": "image/png",
        }));
        let error = prompt_text(&[block]).expect_err("refused");
        let value = error.to_value();
        let reason = value["data"]["prompt"].as_str().expect("reason");
        assert!(reason.contains("image content (image/png)"), "{reason}");
        assert!(reason.contains("text prompts only"), "{reason}");
    }

    #[test]
    fn the_mode_selector_uses_protocol_field_names() {
        let options = config_options(Mode::Auto);
        assert_eq!(options[0]["id"], json!("mode"));
        assert_eq!(options[0]["type"], json!("select"));
        assert_eq!(options[0]["currentValue"], json!("auto"));
        assert_eq!(options[0]["options"][2]["value"], json!("yolo"));
        let modes = modes_state(Mode::Manual);
        assert_eq!(modes["currentModeId"], json!("manual"));
        assert_eq!(modes["availableModes"][1]["id"], json!("auto"));
    }

    #[test]
    fn tool_titles_follow_the_tool_arguments_and_edit_carries_a_diff() {
        let read = present("read_file", &json!({ "path": "src/lib.rs" }));
        assert_eq!(read.title, "Read `src/lib.rs`");
        assert_eq!(read.kind, "read");

        let edit = present(
            "edit_file",
            &json!({ "path": "a.rs", "old": "one", "new": "two" }),
        );
        assert_eq!(edit.title, "Edit `a.rs`");
        assert_eq!(
            edit.content,
            Some(json!([{
                "type": "diff",
                "path": "a.rs",
                "oldText": "one",
                "newText": "two",
            }]))
        );

        let shell = present("shell", &json!({ "command": "cargo test -j 1" }));
        assert_eq!(shell.title, "cargo test -j 1");
        assert_eq!(shell.kind, "execute");

        let other = present("mcp__srv__thing", &json!({}));
        assert_eq!(other.title, "mcp__srv__thing");
        assert_eq!(other.kind, "other");
    }

    #[test]
    fn todo_arguments_become_plan_entries_with_known_statuses() {
        let update = plan_update(&json!({
            "todos": [
                { "content": "read", "status": "completed" },
                { "content": "write", "status": "in_progress" },
                { "content": "test", "status": "whatever" },
            ],
        }))
        .expect("plan");
        assert_eq!(update["sessionUpdate"], json!("plan"));
        assert_eq!(update["entries"][0]["status"], json!("completed"));
        assert_eq!(update["entries"][1]["status"], json!("in_progress"));
        assert_eq!(update["entries"][2]["status"], json!("pending"));
        assert_eq!(update["entries"][0]["priority"], json!("medium"));
        assert!(plan_update(&json!({ "todos": "nope" })).is_none());
        assert!(plan_update(&json!({})).is_none());
    }

    #[test]
    fn a_tool_call_update_omits_content_when_there_is_none() {
        let update = tool_call_update("call-1", "completed", None);
        assert!(update.get("content").is_none());
        let with = tool_call_update("call-1", "failed", Some("no"));
        assert_eq!(with["content"][0]["type"], json!("content"));
        assert_eq!(with["content"][0]["content"]["text"], json!("no"));
    }
}
