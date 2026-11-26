//! Anthropic Messages over SSE.
//!
//! Anthropic splits a tool call into a `content_block_start` that names it and
//! one or more `input_json_delta` frames that carry pieces of the argument
//! object, so the adapter buffers fragments the same way the OpenAI one does.

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use serde_json::{Value, json};

use crate::error::ProviderError;
use crate::sse::{self, SseEvent};
use crate::{ChatRequest, Message, Provider, RetryPolicy, Role, StopReason, StreamEvent, Usage};

/// Endpoint used when the provider block sets no `base_url`.
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

/// API version header value.
pub const API_VERSION: &str = "2023-06-01";

/// A Messages client.
pub struct AnthropicProvider {
    name: String,
    base_url: String,
    api_key: String,
    client: reqwest::Client,
    retry: RetryPolicy,
}

impl AnthropicProvider {
    /// Build a client for `name`.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Transport`] when the HTTP client cannot be
    /// created.
    pub fn new(
        name: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        retry: RetryPolicy,
    ) -> Result<Self, ProviderError> {
        let name = name.into();
        Ok(Self {
            client: sse::client(&name)?,
            name,
            base_url: base_url.into(),
            api_key: api_key.into(),
            retry,
        })
    }

    /// Build a client from a `[providers.<name>]` block.
    ///
    /// # Errors
    ///
    /// As [`AnthropicProvider::new`], plus [`ProviderError::MissingKey`] when
    /// the block names no key variable or the variable is empty.
    pub fn from_config(
        name: &str,
        provider: &niki_config::Provider,
        retry: RetryPolicy,
    ) -> Result<Self, ProviderError> {
        let key = crate::api_key(provider, name)?;
        Self::new(
            name,
            crate::base_url(provider, DEFAULT_BASE_URL),
            key,
            retry,
        )
    }

    fn body(&self, request: &ChatRequest) -> Value {
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.schema,
                })
            })
            .collect();
        json!({
            "model": request.model,
            "max_tokens": request.max_tokens,
            "system": request.system,
            "messages": request.messages.iter().map(encode_message).collect::<Vec<_>>(),
            "tools": tools,
            "stream": true,
        })
    }
}

fn encode_message(message: &Message) -> Value {
    let blocks: Vec<Value> = match message.role {
        Role::Tool => vec![json!({
            "type": "tool_result",
            "tool_use_id": message.tool_call_id.clone().unwrap_or_default(),
            "content": message.content,
        })],
        Role::Assistant => {
            let mut blocks = Vec::with_capacity(message.tool_calls.len() + 1);
            if !message.content.is_empty() {
                blocks.push(json!({"type": "text", "text": message.content}));
            }
            blocks.extend(message.tool_calls.iter().map(|call| {
                json!({
                    "type": "tool_use",
                    "id": call.id,
                    "name": call.name,
                    "input": serde_json::from_str::<Value>(&call.arguments)
                        .unwrap_or_else(|_| json!({})),
                })
            }));
            blocks
        }
        Role::User => vec![json!({"type": "text", "text": message.content})],
    };
    let role = if message.role == Role::Assistant {
        "assistant"
    } else {
        "user"
    };
    json!({"role": role, "content": blocks})
}

impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn stream(
        &self,
        request: ChatRequest,
    ) -> BoxStream<'static, Result<StreamEvent, ProviderError>> {
        let url = format!("{}/v1/messages", self.base_url);
        let body = self.body(&request);
        let client = self.client.clone();
        let key = self.api_key.clone();
        let name = self.name.clone();
        let retry = self.retry;

        let response = async move {
            sse::send_with_retries(name.as_str(), retry, move || {
                Ok(client
                    .post(&url)
                    .header("x-api-key", &key)
                    .header("anthropic-version", API_VERSION)
                    .header("accept", "text/event-stream")
                    .json(&body))
            })
            .await
        };

        let parse_name = self.name.clone();
        let stream = futures_util::stream::once(response).flat_map(move |response| {
            let parse_name = parse_name.clone();
            match response {
                Ok(response) => {
                    let mut state = ParseState::default();
                    sse::stream_events(parse_name.clone(), response, move |event| {
                        state.parse(&parse_name, event)
                    })
                }
                Err(e) => {
                    let stream: BoxStream<'static, Result<StreamEvent, ProviderError>> =
                        Box::pin(futures_util::stream::once(async move { Err(e) }));
                    stream
                }
            }
        });
        Box::pin(stream)
    }
}

#[derive(Debug, Default)]
struct ParseState {
    input_tokens: u64,
    stop: Option<StopReason>,
}

impl ParseState {
    fn parse(
        &mut self,
        provider: &str,
        event: SseEvent,
    ) -> Result<Vec<StreamEvent>, ProviderError> {
        let value: Value =
            serde_json::from_str(&event.data).map_err(|e| ProviderError::Decode {
                provider: provider.to_owned(),
                detail: format!(
                    "{}: {}",
                    e,
                    event.data.chars().take(120).collect::<String>()
                ),
            })?;
        let kind = event
            .event
            .as_deref()
            .or_else(|| value.get("type").and_then(Value::as_str))
            .unwrap_or_default();
        match kind {
            "message_start" => {
                if let Some(tokens) = value
                    .get("message")
                    .and_then(|m| m.get("usage"))
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(Value::as_u64)
                {
                    self.input_tokens = tokens;
                }
                Ok(Vec::new())
            }
            "content_block_start" => {
                let Some(block) = value.get("content_block") else {
                    return Ok(Vec::new());
                };
                if block.get("type").and_then(Value::as_str) != Some("tool_use") {
                    return Ok(Vec::new());
                }
                Ok(vec![StreamEvent::ToolCallDelta {
                    index: value.get("index").and_then(Value::as_u64).unwrap_or(0) as usize,
                    id: block.get("id").and_then(Value::as_str).map(str::to_owned),
                    name: block.get("name").and_then(Value::as_str).map(str::to_owned),
                    arguments: String::new(),
                }])
            }
            "content_block_delta" => {
                let Some(delta) = value.get("delta") else {
                    return Ok(Vec::new());
                };
                match delta.get("type").and_then(Value::as_str) {
                    Some("text_delta") => Ok(delta
                        .get("text")
                        .and_then(Value::as_str)
                        .filter(|t| !t.is_empty())
                        .map(|t| vec![StreamEvent::TextDelta(t.to_owned())])
                        .unwrap_or_default()),
                    Some("input_json_delta") => Ok(vec![StreamEvent::ToolCallDelta {
                        index: value.get("index").and_then(Value::as_u64).unwrap_or(0) as usize,
                        id: None,
                        name: None,
                        arguments: delta
                            .get("partial_json")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    }]),
                    _ => Ok(Vec::new()),
                }
            }
            "message_delta" => {
                if let Some(reason) = value
                    .get("delta")
                    .and_then(|d| d.get("stop_reason"))
                    .and_then(Value::as_str)
                {
                    self.stop = Some(match reason {
                        "end_turn" | "stop_sequence" => StopReason::EndTurn,
                        "tool_use" => StopReason::ToolUse,
                        "max_tokens" => StopReason::MaxTokens,
                        other => StopReason::Other(other.to_owned()),
                    });
                }
                let output = value
                    .get("usage")
                    .and_then(|u| u.get("output_tokens"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                Ok(vec![StreamEvent::Usage(Usage {
                    input_tokens: self.input_tokens,
                    output_tokens: output,
                })])
            }
            "message_stop" => Ok(vec![StreamEvent::Done(
                self.stop.clone().unwrap_or(StopReason::EndTurn),
            )]),
            "error" => Err(ProviderError::Decode {
                provider: provider.to_owned(),
                detail: value
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("stream error")
                    .to_owned(),
            }),
            _ => Ok(Vec::new()),
        }
    }
}
