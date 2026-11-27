//! Model provider clients: OpenAI-compatible Chat Completions and Anthropic
//! Messages, streamed over SSE.
//!
//! ADR-004: the Rust client talks to the same endpoints the Python product
//! talks to, with the same request shapes, so a user's existing provider block
//! works unchanged. Two adapters cover every built-in provider: everything
//! except Anthropic speaks the OpenAI shape.
//!
//! A turn is a stream of [`StreamEvent`]s. Tool calls arrive as fragments; the
//! assembler in each adapter turns them back into one [`ToolCall`] because the
//! wire formats split an argument object across chunks.

#![forbid(unsafe_code)]

use std::fmt;

use futures_util::stream::BoxStream;
use serde_json::Value;

pub mod anthropic;
pub mod error;
pub mod openai;
pub mod retry;
pub mod sse;

pub use anthropic::AnthropicProvider;
pub use error::ProviderError;
pub use openai::OpenAiProvider;
pub use retry::RetryPolicy;

/// Who produced a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The person running the agent.
    User,
    /// The model.
    Assistant,
    /// A tool result, which both wire formats attach to a tool call id.
    Tool,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        })
    }
}

/// One tool call the model asked for.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToolCall {
    /// Provider-assigned id, echoed back with the result.
    pub id: String,
    /// Tool name as the registry knows it.
    pub name: String,
    /// Arguments as the model sent them: a JSON object in a string.
    pub arguments: String,
}

/// One message in the conversation sent to the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Who produced it.
    pub role: Role,
    /// Text content; empty for an assistant message that only calls tools.
    pub content: String,
    /// Tool calls this message carries. Only assistant messages use it.
    pub tool_calls: Vec<ToolCall>,
    /// The call this message answers. Only tool messages use it.
    pub tool_call_id: Option<String>,
}

impl Message {
    /// A user message.
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    /// An assistant message with plain text and no tool calls.
    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    /// A tool result answering `call`.
    #[must_use]
    pub fn tool_result(call: &ToolCall, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: Some(call.id.clone()),
        }
    }
}

/// One tool the model may call.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    /// Name the model uses.
    pub name: String,
    /// What the tool does, for the model.
    pub description: String,
    /// JSON Schema for the arguments object.
    pub schema: Value,
}

/// One request for one model turn.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatRequest {
    /// Model identifier as the provider names it.
    pub model: String,
    /// System prompt, sent separately by both wire formats.
    pub system: String,
    /// Conversation so far.
    pub messages: Vec<Message>,
    /// Tools the model may call.
    pub tools: Vec<ToolSpec>,
    /// Upper bound on the answer.
    pub max_tokens: u32,
}

/// Token accounting for the turn, when the provider reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    /// Prompt tokens.
    pub input_tokens: u64,
    /// Completion tokens.
    pub output_tokens: u64,
}

/// Why the model stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// The model finished its answer.
    EndTurn,
    /// The model wants tool results before continuing.
    ToolUse,
    /// The token budget ran out.
    MaxTokens,
    /// Anything else the provider reported.
    Other(String),
}

/// One event from a streaming turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    /// A piece of answer text, to append to the transcript.
    TextDelta(String),
    /// A piece of a tool call, or the start of one.
    ToolCallDelta {
        /// Position in the request's tool-call list.
        index: usize,
        /// Set on the first fragment and repeated by some providers.
        id: Option<String>,
        /// Set on the first fragment.
        name: Option<String>,
        /// Argument text appended to this call's buffer.
        arguments: String,
    },
    /// Token accounting, usually the last event before the stop.
    Usage(Usage),
    /// The turn is over.
    Done(StopReason),
}

/// A streaming model client.
///
/// Implementations are `Send + Sync` so a turn can run on a worker while the
/// UI reads it.
pub trait Provider: Send + Sync {
    /// Name used in error messages and logs.
    fn name(&self) -> &str;

    /// Run one turn, returning its events in arrival order.
    ///
    /// The stream ends after a [`StreamEvent::Done`]; a transport or decode
    /// failure ends it with an [`ProviderError`] instead.
    fn stream(
        &self,
        request: ChatRequest,
    ) -> BoxStream<'static, Result<StreamEvent, ProviderError>>;
}

/// The base URL to use for `provider`, or `default` when the block does not set
/// one.
#[must_use]
pub fn base_url(provider: &niki_config::Provider, default: &str) -> String {
    provider.base_url.as_deref().map_or_else(
        || default.to_owned(),
        |url| url.trim_end_matches('/').to_owned(),
    )
}

/// The API key named by `provider`, read from the environment.
///
/// # Errors
///
/// Returns [`ProviderError::MissingKey`] when the variable is unset or empty,
/// naming both the variable and the provider so the user can fix the shell.
pub fn api_key(provider: &niki_config::Provider, name: &str) -> Result<String, ProviderError> {
    // A local daemon ignores the credential it is sent. If the user named a
    // variable and set it anyway (a proxy in front of the daemon), use it.
    if provider.keyless() {
        return Ok(provider
            .api_key_env
            .as_deref()
            .map(str::trim)
            .filter(|var| !var.is_empty())
            .and_then(|var| std::env::var(var).ok())
            .unwrap_or_default());
    }
    let var = provider
        .api_key_env
        .as_deref()
        .ok_or_else(|| ProviderError::MissingKey {
            provider: name.to_owned(),
            var: "(none configured)".to_owned(),
        })?;
    let value = std::env::var(var).unwrap_or_default();
    if value.trim().is_empty() {
        return Err(ProviderError::MissingKey {
            provider: name.to_owned(),
            var: var.to_owned(),
        });
    }
    Ok(value)
}
