//! Context accounting: what the session has spent, and what the conversation
//! currently occupies.
//!
//! The token figures here are estimates. A provider reports exact usage for a
//! request it has seen; before sending one there is nothing to report with, so
//! [`estimate_tokens`] divides the text's character count by a fixed budget and
//! says so.

use niki_provider::{Message, Role};
use niki_tools::Registry;
use serde_json::Value;

/// Characters attributed to one token by [`estimate_tokens`].
const CHARS_PER_TOKEN: usize = 4;

/// Spending accumulated over every run of one agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SessionUsage {
    /// Prompt tokens the provider reported.
    pub input_tokens: u64,
    /// Completion tokens the provider reported.
    pub output_tokens: u64,
    /// Model turns taken.
    pub steps: u32,
}

impl SessionUsage {
    /// Input and output tokens together.
    #[must_use]
    pub fn total_tokens(self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

/// The conversation's size and what the session has spent so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContextReport {
    /// Messages in the conversation.
    pub messages: usize,
    /// Estimated prompt tokens for the whole conversation, not counting the
    /// system prompt or the tool schemas the provider adds.
    pub estimated_tokens: usize,
    /// Prompt tokens the provider reported for the most recent model turn.
    pub last_input_tokens: u64,
    /// Completion tokens the provider reported for the most recent model turn.
    pub last_output_tokens: u64,
    /// Model turns taken over every run of this agent.
    pub steps: u32,
}

/// What one request would inject, and what it is estimated to cost.
///
/// A provider adds the system prompt and the tool array to every request;
/// [`ContextReport`] accounts only for the conversation. An audit splits the
/// three parts so a session can show where its prompt budget goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContextAudit {
    /// Estimated tokens in the system prompt.
    pub system_tokens: usize,
    /// Estimated tokens in the tool array: each tool's name, description and
    /// serialised parameter schema.
    pub tool_schema_tokens: usize,
    /// Estimated tokens in the conversation.
    pub message_tokens: usize,
    /// Tools the request would declare.
    pub tool_count: usize,
}

impl ContextAudit {
    /// System prompt, tool array and conversation together.
    #[must_use]
    pub fn total_tokens(self) -> usize {
        self.system_tokens + self.tool_schema_tokens + self.message_tokens
    }
}

/// Audit what a request carrying `system`, `messages` and `tools` would inject.
///
/// The figures use the same character budget as [`estimate_tokens`]. Each tool
/// is charged for its name, its description and its parameter schema
/// serialised as JSON, laid end to end.
#[must_use]
pub fn audit_context(system: &str, messages: &[Message], tools: &Registry) -> ContextAudit {
    let (tool_schema_tokens, tool_count) = match tools.specs() {
        Value::Array(items) => {
            let tokens = items
                .iter()
                .map(|item| {
                    let name = item.get("name").and_then(Value::as_str).unwrap_or_default();
                    let description = item
                        .get("description")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let parameters = item
                        .get("parameters")
                        .map(|schema| serde_json::to_string(schema).unwrap_or_default())
                        .unwrap_or_default();
                    estimate_text_tokens(&format!("{name}{description}{parameters}"))
                })
                .sum();
            (tokens, items.len())
        }
        _ => (0, 0),
    };

    ContextAudit {
        system_tokens: estimate_text_tokens(system),
        tool_schema_tokens,
        message_tokens: estimate_tokens(messages),
        tool_count,
    }
}

/// What one compaction did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Compaction {
    /// Messages replaced by the summary.
    pub replaced: usize,
    /// Estimated tokens in the assistant message that replaced them.
    pub summary_tokens: usize,
}

/// Estimate the prompt tokens `messages` occupies.
///
/// The text of every message, plus the names and arguments of its tool calls,
/// is counted in characters and divided by four, rounded up. It is an
/// estimate: a real tokeniser splits on sub-word boundaries, and the wire
/// format adds per-message overhead this does not model.
#[must_use]
pub fn estimate_tokens(messages: &[Message]) -> usize {
    let chars: usize = messages
        .iter()
        .map(|message| {
            message.content.chars().count()
                + message
                    .tool_calls
                    .iter()
                    .map(|call| call.name.chars().count() + call.arguments.chars().count())
                    .sum::<usize>()
        })
        .sum();
    estimate_text_tokens_of(chars)
}

/// Estimate the tokens in one string, with the same rule as [`estimate_tokens`].
pub(crate) fn estimate_text_tokens(text: &str) -> usize {
    estimate_text_tokens_of(text.chars().count())
}

fn estimate_text_tokens_of(chars: usize) -> usize {
    chars.div_ceil(CHARS_PER_TOKEN)
}

/// The first index of the last `keep_last` turns, or zero when there is
/// nothing older to fold.
///
/// A turn starts at a user message; messages before the first user message —
/// the summary a previous compaction left — form a turn of their own.
pub(crate) fn keep_from(messages: &[Message], keep_last: usize) -> usize {
    let mut starts = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        if index == 0 || message.role == Role::User {
            starts.push(index);
        }
    }
    match starts.len().checked_sub(keep_last) {
        None => 0,
        Some(split) if split < starts.len() => starts[split],
        Some(_) => messages.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use niki_provider::ToolCall;

    fn turn(id: &str) -> Vec<Message> {
        vec![
            Message::user(format!("ask {id}")),
            Message::assistant(format!("answer {id}")),
        ]
    }

    #[test]
    fn the_estimate_counts_text_and_tool_arguments() {
        let mut message = Message::assistant("abcd");
        message.tool_calls.push(ToolCall {
            id: "call".to_owned(),
            name: "read".to_owned(),
            arguments: "{}".to_owned(),
        });
        assert_eq!(estimate_tokens(std::slice::from_ref(&message)), 3);
        assert_eq!(estimate_tokens(&[]), 0);
    }

    #[test]
    fn turn_boundaries_start_at_user_messages() {
        let messages = turn("one")
            .into_iter()
            .chain(turn("two"))
            .collect::<Vec<_>>();
        assert_eq!(keep_from(&messages, 1), 2);
        assert_eq!(keep_from(&messages, 2), 0);
        assert_eq!(keep_from(&messages, 3), 0);
        assert_eq!(keep_from(&messages, 0), 4);
    }

    #[test]
    fn a_leading_summary_is_a_turn_of_its_own() {
        let mut messages = vec![Message::assistant("summary")];
        messages.extend(turn("one"));
        messages.extend(turn("two"));
        assert_eq!(keep_from(&messages, 1), 3);
        assert_eq!(keep_from(&messages, 2), 1);
    }
}
