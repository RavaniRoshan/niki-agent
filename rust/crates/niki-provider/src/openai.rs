//! OpenAI-compatible Chat Completions over SSE.
//!
//! Covers every built-in provider except Anthropic: OpenAI, Google's
//! OpenAI-compatible endpoint, OpenRouter, Groq, DeepSeek, xAI, and any custom
//! block that sets `base_url`.

use std::collections::BTreeMap;

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use serde_json::{Value, json};

use crate::error::ProviderError;
use crate::sse::{self, SseEvent};
use crate::{
    ChatRequest, Message, Provider, RetryPolicy, Role, StopReason, StreamEvent, ToolCall, Usage,
};

/// Endpoint used when the provider block sets no `base_url`.
pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

/// A Chat Completions client.
pub struct OpenAiProvider {
    name: String,
    base_url: String,
    api_key: String,
    client: reqwest::Client,
    retry: RetryPolicy,
}

impl OpenAiProvider {
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
    /// As [`OpenAiProvider::new`], plus [`ProviderError::MissingKey`] when the
    /// block names no key variable or the variable is empty.
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
        let mut messages = Vec::with_capacity(request.messages.len() + 1);
        if !request.system.is_empty() {
            messages.push(json!({"role": "system", "content": request.system}));
        }
        messages.extend(request.messages.iter().map(encode_message));
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.schema,
                    }
                })
            })
            .collect();
        json!({
            "model": request.model,
            "messages": messages,
            "tools": tools,
            "max_tokens": request.max_tokens,
            "stream": true,
            "stream_options": {"include_usage": true},
        })
    }
}

fn encode_message(message: &Message) -> Value {
    match message.role {
        Role::Tool => json!({
            "role": "tool",
            "tool_call_id": message.tool_call_id.clone().unwrap_or_default(),
            "content": message.content,
        }),
        Role::Assistant if !message.tool_calls.is_empty() => json!({
            "role": "assistant",
            "content": message.content,
            "tool_calls": message.tool_calls.iter().map(|call| json!({
                "id": call.id,
                "type": "function",
                "function": {"name": call.name, "arguments": call.arguments},
            })).collect::<Vec<_>>(),
        }),
        role => json!({"role": role.to_string(), "content": message.content}),
    }
}

impl Provider for OpenAiProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn stream(
        &self,
        request: ChatRequest,
    ) -> BoxStream<'static, Result<StreamEvent, ProviderError>> {
        let url = format!("{}/chat/completions", self.base_url);
        let body = self.body(&request);
        let client = self.client.clone();
        let key = self.api_key.clone();
        let name = self.name.clone();
        let retry = self.retry;

        let response = async move {
            sse::send_with_retries(name.as_str(), retry, move || {
                Ok(client
                    .post(&url)
                    .bearer_auth(&key)
                    .header("accept", "text/event-stream")
                    .json(&body))
            })
            .await
        };

        let parse_name = self.name.clone();
        let stream = futures_util::stream::once(response).flat_map(move |response| {
            let parse_name = parse_name.clone();
            match response {
                Ok(response) => sse::stream_events(parse_name.clone(), response, move |event| {
                    parse_frame(&parse_name, event)
                }),
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

/// Parse one OpenAI chunk.
///
/// Tool calls arrive as fragments keyed by `index`; callers concatenate
/// `arguments` across events, which is why the fragment carries the index.
fn parse_frame(provider: &str, event: SseEvent) -> Result<Vec<StreamEvent>, ProviderError> {
    let value: Value = serde_json::from_str(&event.data).map_err(|e| ProviderError::Decode {
        provider: provider.to_owned(),
        detail: format!(
            "{}: {}",
            e,
            event.data.chars().take(120).collect::<String>()
        ),
    })?;
    let mut events = Vec::new();
    let choices = value
        .get("choices")
        .and_then(Value::as_array)
        .filter(|c| !c.is_empty());
    let Some(choice) = choices.and_then(|c| c.first()) else {
        if let Some(usage) = usage_of(&value) {
            events.push(StreamEvent::Usage(usage));
        }
        return Ok(events);
    };
    if let Some(delta) = choice.get("delta") {
        if let Some(text) = delta.get("content").and_then(Value::as_str) {
            if !text.is_empty() {
                events.push(StreamEvent::TextDelta(text.to_owned()));
            }
        }
        if let Some(call) = delta
            .get("tool_calls")
            .and_then(Value::as_array)
            .and_then(|c| c.first())
        {
            events.push(StreamEvent::ToolCallDelta {
                index: call.get("index").and_then(Value::as_u64).unwrap_or(0) as usize,
                id: call.get("id").and_then(Value::as_str).map(str::to_owned),
                name: call
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                arguments: call
                    .get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            });
        }
    }
    if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
        events.push(StreamEvent::Done(match reason {
            "stop" => StopReason::EndTurn,
            "tool_calls" | "function_call" => StopReason::ToolUse,
            "length" => StopReason::MaxTokens,
            other => StopReason::Other(other.to_owned()),
        }));
    }
    if let Some(usage) = usage_of(&value) {
        events.push(StreamEvent::Usage(usage));
    }
    Ok(events)
}

fn usage_of(value: &Value) -> Option<Usage> {
    let usage = value.get("usage").filter(|u| !u.is_null())?;
    Some(Usage {
        input_tokens: usage
            .get("prompt_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        output_tokens: usage
            .get("completion_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    })
}

/// Accumulates tool-call fragments into whole calls.
#[derive(Debug, Default)]
pub struct ToolCallAssembler {
    calls: BTreeMap<usize, ToolCall>,
}

impl ToolCallAssembler {
    /// Apply one fragment.
    pub fn push(
        &mut self,
        index: usize,
        id: Option<String>,
        name: Option<String>,
        arguments: &str,
    ) {
        let call = self.calls.entry(index).or_default();
        if let Some(id) = id {
            call.id = id;
        }
        if let Some(name) = name {
            call.name = name;
        }
        call.arguments.push_str(arguments);
    }

    /// Consume the assembler, returning calls in request order.
    #[must_use]
    pub fn finish(self) -> Vec<ToolCall> {
        self.calls.into_values().collect()
    }
}
