use std::sync::Arc;

use niki_tools::{Tool, ToolContext, ToolError, ToolOutput};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{McpError, Server};

/// Provider tool names have a strict character set and a length cap.
const MAX_TOOL_NAME: usize = 64;

/// A tool one MCP server advertised from `tools/list`.
#[derive(Debug, Clone, PartialEq)]
pub struct McpTool {
    /// Server-side name, sent back in `tools/call`.
    pub name: String,
    /// One sentence the model reads.
    pub description: String,
    /// JSON Schema of the argument object.
    pub input_schema: Value,
    /// Whether the server declared the tool read-only: `readOnlyHint` is `true`
    /// and `destructiveHint` is not. Only a coherent declaration counts, the
    /// same rule the Python client applies.
    pub read_only: bool,
}

/// The provider-visible name of an MCP tool: `mcp__<server>__<tool>`.
///
/// The prefix keeps every MCP name away from a built-in, and the double
/// underscore is the name the Python client uses in hooks and matchers, so a
/// saved matcher works in both clients. Characters outside `[A-Za-z0-9_-]`
/// become `_`; a name longer than the provider cap keeps a stable digest.
#[must_use]
pub fn tool_name(server: &str, tool: &str) -> String {
    let base = format!("mcp__{}__{}", sanitize(server), sanitize(tool));
    if base.len() <= MAX_TOOL_NAME {
        return base;
    }
    let digest = fnv1a(format!("{server}\0{tool}").as_bytes());
    let tail = format!("__{digest:012x}");
    let keep = MAX_TOOL_NAME - tail.len();
    format!("{}{tail}", &base[..keep])
}

/// Replace every run of characters a provider does not accept with `_`.
fn sanitize(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut in_run = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
            out.push(character);
            in_run = false;
        } else if !in_run {
            out.push('_');
            in_run = true;
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "unnamed".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// FNV-1a, so a truncated name is stable across runs and machines. The Python
/// client uses SHA-256; this client only needs the same input to yield the
/// same name every time.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[derive(Debug, Deserialize)]
struct ListResult {
    tools: Vec<RawTool>,
}

#[derive(Debug, Deserialize)]
struct RawTool {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "object_schema", rename = "inputSchema")]
    input_schema: Value,
    #[serde(default)]
    annotations: RawAnnotations,
}

#[derive(Debug, Default, Deserialize)]
struct RawAnnotations {
    #[serde(default, rename = "readOnlyHint")]
    read_only: Option<Value>,
    #[serde(default, rename = "destructiveHint")]
    destructive: Option<Value>,
}

impl RawAnnotations {
    fn coherently_read_only(&self) -> bool {
        self.read_only == Some(Value::Bool(true)) && self.destructive != Some(Value::Bool(true))
    }
}

fn object_schema() -> Value {
    json!({"type": "object"})
}

/// Parse the result of `tools/list`.
pub(crate) fn parse_tools(server: &str, result: &Value) -> Result<Vec<McpTool>, McpError> {
    let list: ListResult =
        serde_json::from_value(result.clone()).map_err(|error| McpError::Malformed {
            server: server.to_owned(),
            method: "tools/list".to_owned(),
            detail: error.to_string(),
        })?;
    Ok(list
        .tools
        .into_iter()
        .map(|raw| McpTool {
            name: raw.name,
            description: raw.description,
            input_schema: raw.input_schema,
            read_only: raw.annotations.coherently_read_only(),
        })
        .collect())
}

/// What `tools/call` returned, rendered for the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallResult {
    /// Text built from the result's content blocks, or its structured content
    /// when the server sent no text blocks.
    pub text: String,
    /// Whether the server marked the result as a failure (`isError`).
    pub is_error: bool,
}

impl CallResult {
    pub(crate) fn parse(server: &str, result: &Value) -> Result<Self, McpError> {
        if !result.is_object() {
            return Err(McpError::Malformed {
                server: server.to_owned(),
                method: "tools/call".to_owned(),
                detail: "the result is not an object".to_owned(),
            });
        }
        let content = result
            .get("content")
            .map_or_else(String::new, render_content);
        let text = if content.is_empty() {
            result
                .get("structuredContent")
                .map_or_else(String::new, |value| {
                    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
                })
        } else {
            content
        };
        Ok(Self {
            text,
            is_error: result
                .get("isError")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }
}

fn render_content(content: &Value) -> String {
    let Value::Array(blocks) = content else {
        return String::new();
    };
    blocks
        .iter()
        .map(render_block)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_block(block: &Value) -> String {
    match block.get("type").and_then(Value::as_str) {
        Some("text") => block
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        Some("image") => format!("[image: {}]", media_type(block)),
        Some("audio") => format!("[audio: {}]", media_type(block)),
        Some("resource") => block
            .get("resource")
            .map_or_else(|| "[resource]".to_owned(), render_resource),
        Some(other) => format!("[{other}]"),
        None => block.to_string(),
    }
}

fn media_type(block: &Value) -> &str {
    block
        .get("mimeType")
        .and_then(Value::as_str)
        .unwrap_or("unknown type")
}

fn render_resource(resource: &Value) -> String {
    if let Some(text) = resource.get("text").and_then(Value::as_str) {
        return text.to_owned();
    }
    resource.get("uri").and_then(Value::as_str).map_or_else(
        || "[resource]".to_owned(),
        |uri| format!("[resource: {uri}]"),
    )
}

/// One MCP tool adapted to the registry's [`Tool`] trait.
///
/// The bridge keeps the owning server alive, so a registry built from
/// [`Server::bridges`](crate::Server::bridges) holds the connection open for as
/// long as any of its tools is registered. Cloning a bridge shares that one
/// connection: a host that rebuilds its agent, as the screen does on a model
/// switch, can register the same tools again without a second handshake.
#[derive(Clone)]
pub struct ToolBridge {
    server: Arc<Server>,
    tool: McpTool,
    name: String,
}

impl ToolBridge {
    pub(crate) fn new(server: Arc<Server>, tool: McpTool) -> Self {
        let name = tool_name(server.name(), &tool.name);
        Self { server, tool, name }
    }

    /// The configured server name.
    #[must_use]
    pub fn server(&self) -> &str {
        self.server.name()
    }

    /// The server-side tool name, before the prefix.
    #[must_use]
    pub fn tool(&self) -> &str {
        &self.tool.name
    }
}

impl Tool for ToolBridge {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.tool.description
    }

    fn schema(&self) -> Value {
        self.tool.input_schema.clone()
    }

    fn requires_approval(&self) -> bool {
        !self.tool.read_only
    }

    async fn run(&self, args: Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let result = self
            .server
            .call_tool(&self.tool.name, args)
            .await
            .map_err(|error| ToolError::Mcp(error.to_string()))?;
        if result.is_error {
            return Err(ToolError::Mcp(format!(
                "mcp server `{}` tool `{}` reported an error: {}",
                self.server.name(),
                self.tool.name,
                result.text
            )));
        }
        Ok(ToolOutput::new(result.text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_prefixed_and_sanitized() {
        assert_eq!(tool_name("fs", "read_file"), "mcp__fs__read_file");
        assert_eq!(
            tool_name("my server", "read file"),
            "mcp__my_server__read_file"
        );
        assert_eq!(tool_name("", "!!"), "mcp__unnamed__unnamed");
        assert_eq!(tool_name("a.b", "c/d"), "mcp__a_b__c_d");
    }

    #[test]
    fn a_long_name_is_capped_with_a_stable_digest() {
        let tool = "x".repeat(80);
        let first = tool_name("server", &tool);
        let second = tool_name("server", &tool);
        assert_eq!(first, second);
        assert!(
            first.len() <= MAX_TOOL_NAME,
            "{first} is {} bytes",
            first.len()
        );
        assert!(first.starts_with("mcp__server__xxx"));
    }

    #[test]
    fn tools_list_reads_the_schema_and_the_read_only_hint() -> Result<(), McpError> {
        let result = json!({
            "tools": [
                {
                    "name": "echo",
                    "description": "Echo the message back.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {"message": {"type": "string"}},
                        "required": ["message"]
                    }
                },
                {
                    "name": "peek",
                    "annotations": {"readOnlyHint": true}
                },
                {
                    "name": "delete",
                    "annotations": {"readOnlyHint": true, "destructiveHint": true}
                },
                {
                    "name": "loose",
                    "annotations": {"readOnlyHint": "true"}
                }
            ]
        });
        let tools = parse_tools("fixture", &result)?;
        assert_eq!(tools[0].name, "echo");
        assert_eq!(tools[0].description, "Echo the message back.");
        assert_eq!(
            tools[0].input_schema["properties"]["message"]["type"],
            "string"
        );
        assert!(!tools[0].read_only, "no hint is not read-only");
        assert!(tools[1].read_only);
        assert_eq!(tools[1].input_schema, json!({"type": "object"}));
        assert!(
            !tools[2].read_only,
            "a destructive hint overrides read-only"
        );
        assert!(!tools[3].read_only, "a non-boolean hint is not coherent");
        Ok(())
    }

    #[test]
    fn a_tools_list_without_a_tools_array_is_malformed() {
        let error = parse_tools("fixture", &json!({"nope": []})).expect_err("error");
        assert!(matches!(error, McpError::Malformed { .. }), "{error:?}");
        let text = error.to_string();
        assert!(text.contains("fixture"), "{text}");
        assert!(text.contains("tools/list"), "{text}");
    }

    #[test]
    fn a_call_result_renders_text_blocks_and_the_error_flag() -> Result<(), McpError> {
        let result = json!({
            "content": [
                {"type": "text", "text": "first"},
                {"type": "image", "mimeType": "image/png", "data": "…"},
                {"type": "resource", "resource": {"uri": "file:///tmp/a.txt", "text": "file body"}}
            ],
            "isError": true
        });
        let parsed = CallResult::parse("fixture", &result)?;
        assert_eq!(parsed.text, "first\n[image: image/png]\nfile body");
        assert!(parsed.is_error);
        Ok(())
    }

    #[test]
    fn a_call_result_falls_back_to_structured_content() -> Result<(), McpError> {
        let result = json!({
            "content": [],
            "structuredContent": {"answer": 42}
        });
        let parsed = CallResult::parse("fixture", &result)?;
        assert!(parsed.text.contains("\"answer\": 42"), "{}", parsed.text);
        assert!(!parsed.is_error);
        Ok(())
    }
}
