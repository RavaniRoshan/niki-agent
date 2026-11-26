//! Engine stream protocol types and committed replay fixtures.
//!
//! This crate owns the wire vocabulary the engine serves: frame names,
//! incremental SSE parsing, the fixture container, and the schema test that
//! rejects an event the client does not understand.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FIXTURE_FORMAT: &str = "niki.protocol.fixture.v1";

/// Every stream event the engine has emitted for a Niki run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EventName {
    #[serde(rename = "metadata")]
    Metadata,
    #[serde(rename = "updates")]
    Updates,
    #[serde(rename = "values")]
    Values,
    #[serde(rename = "messages/complete")]
    MessagesComplete,
    #[serde(rename = "messages/metadata")]
    MessagesMetadata,
    #[serde(rename = "messages/partial")]
    MessagesPartial,
    #[serde(rename = "messages/tuple")]
    MessagesTuple,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "end")]
    End,
    #[serde(rename = "events")]
    Events,
}

impl EventName {
    /// Parse a wire event name.
    ///
    /// # Errors
    ///
    /// Returns [`ProtoError::UnknownEvent`] for a name this client cannot
    /// interpret, so a protocol addition fails loudly instead of being dropped.
    pub fn from_wire(name: &str) -> Result<Self, ProtoError> {
        match name {
            "metadata" => Ok(Self::Metadata),
            "updates" => Ok(Self::Updates),
            "values" => Ok(Self::Values),
            "messages/complete" => Ok(Self::MessagesComplete),
            "messages/metadata" => Ok(Self::MessagesMetadata),
            "messages/partial" => Ok(Self::MessagesPartial),
            "messages/tuple" => Ok(Self::MessagesTuple),
            "error" => Ok(Self::Error),
            "end" => Ok(Self::End),
            "events" => Ok(Self::Events),
            other => Err(ProtoError::UnknownEvent(other.to_owned())),
        }
    }

    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::Updates => "updates",
            Self::Values => "values",
            Self::MessagesComplete => "messages/complete",
            Self::MessagesMetadata => "messages/metadata",
            Self::MessagesPartial => "messages/partial",
            Self::MessagesTuple => "messages/tuple",
            Self::Error => "error",
            Self::End => "end",
            Self::Events => "events",
        }
    }
}

impl fmt::Display for EventName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_wire())
    }
}

/// One parsed SSE frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub event: EventName,
    pub data: Value,
}

/// One SSE frame before the event name is validated.
#[derive(Debug, Clone, PartialEq)]
pub struct RawFrame {
    pub event: String,
    pub data: Value,
    pub id: Option<String>,
}

/// A normalized protocol capture that the renderer can replay offline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transcript {
    pub format: String,
    pub name: String,
    #[serde(default)]
    pub provenance: BTreeMap<String, Value>,
    #[serde(default)]
    pub request: Value,
    pub frames: Vec<Frame>,
}

impl Transcript {
    /// Parse and validate a fixture document.
    ///
    /// # Errors
    ///
    /// Returns an error when the format tag is wrong, a frame is malformed, or
    /// an event name is unknown.
    pub fn from_json(input: &str) -> Result<Self, ProtoError> {
        let transcript: Self =
            serde_json::from_str(input).map_err(|error| ProtoError::Json(error.to_string()))?;
        if transcript.format != FIXTURE_FORMAT {
            return Err(ProtoError::Format {
                expected: FIXTURE_FORMAT.to_owned(),
                actual: transcript.format,
            });
        }
        Ok(transcript)
    }

    /// Parse raw SSE text into frames, validating every event name.
    ///
    /// # Errors
    ///
    /// Returns an error for a frame with no `data:` line, invalid JSON, or an
    /// unknown event.
    pub fn from_sse(name: &str, input: &str) -> Result<Self, ProtoError> {
        let frames = parse_sse(input)?
            .into_iter()
            .map(|raw| {
                Ok(Frame {
                    event: EventName::from_wire(&raw.event)?,
                    data: raw.data,
                })
            })
            .collect::<Result<Vec<_>, ProtoError>>()?;
        Ok(Self {
            format: FIXTURE_FORMAT.to_owned(),
            name: name.to_owned(),
            provenance: BTreeMap::new(),
            request: Value::Null,
            frames,
        })
    }
}

/// Parse SSE text into frames without interpreting event names.
///
/// A `data:` line may repeat; the values are joined with a newline per the SSE
/// specification. Comment lines and unknown fields are ignored.
///
/// # Errors
///
/// Returns [`ProtoError::MalformedFrame`] when a block has no event or no data,
/// and [`ProtoError::Json`] when a data line is not JSON.
pub fn parse_sse(input: &str) -> Result<Vec<RawFrame>, ProtoError> {
    let mut frames = Vec::new();
    let mut event: Option<String> = None;
    let mut data = String::new();
    let mut id: Option<String> = None;

    let mut flush = |event: &mut Option<String>, data: &mut String, id: &mut Option<String>| {
        if event.is_none() && data.is_empty() && id.is_none() {
            return Ok(());
        }
        let name = event
            .take()
            .ok_or_else(|| ProtoError::MalformedFrame("frame has no event name".to_owned()))?;
        if data.is_empty() {
            return Err(ProtoError::MalformedFrame(format!(
                "frame {name} has no data"
            )));
        }
        let value = serde_json::from_str(data.trim())
            .map_err(|error| ProtoError::Json(format!("frame {name}: {error}")))?;
        frames.push(RawFrame {
            event: name,
            data: value,
            id: id.take(),
        });
        data.clear();
        Ok(())
    };

    for line in input.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            flush(&mut event, &mut data, &mut id)?;
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        let (field, value) = line
            .split_once(':')
            .ok_or_else(|| ProtoError::MalformedFrame(format!("no field separator: {line}")))?;
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => event = Some(value.to_owned()),
            "data" => {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(value);
            }
            "id" => id = Some(value.to_owned()),
            _ => {}
        }
    }
    flush(&mut event, &mut data, &mut id)?;
    Ok(frames)
}

/// Read `content` from an engine message object.
#[must_use]
pub fn message_content(value: &Value) -> Option<String> {
    match value.get("content")? {
        Value::String(text) => Some(text.clone()),
        Value::Array(blocks) => {
            let mut text = String::new();
            for block in blocks {
                match block {
                    Value::String(piece) => text.push_str(piece),
                    Value::Object(map) => {
                        if let Some(Value::String(piece)) = map.get("text") {
                            text.push_str(piece);
                        }
                    }
                    _ => {}
                }
            }
            Some(text)
        }
        _ => None,
    }
}

/// The role carried by an engine message object.
#[must_use]
pub fn message_role(value: &Value) -> &str {
    value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

#[derive(Debug, thiserror::Error)]
pub enum ProtoError {
    #[error("unknown protocol event: {0}")]
    UnknownEvent(String),
    #[error("malformed SSE frame: {0}")]
    MalformedFrame(String),
    #[error("invalid JSON: {0}")]
    Json(String),
    #[error("fixture format is {actual}, expected {expected}")]
    Format { expected: String, actual: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO: &str = include_str!("../fixtures/hello.json");

    #[test]
    fn every_wire_name_round_trips() {
        for name in [
            EventName::Metadata,
            EventName::Updates,
            EventName::Values,
            EventName::MessagesComplete,
            EventName::MessagesMetadata,
            EventName::MessagesPartial,
            EventName::MessagesTuple,
            EventName::Error,
            EventName::End,
            EventName::Events,
        ] {
            assert_eq!(EventName::from_wire(name.as_wire()).unwrap(), name);
        }
    }

    #[test]
    fn unknown_event_is_rejected() {
        let error = EventName::from_wire("messages/delta").unwrap_err();
        assert!(matches!(error, ProtoError::UnknownEvent(name) if name == "messages/delta"));
    }

    #[test]
    fn hello_fixture_parses_and_replays_deterministically() {
        let first = Transcript::from_json(HELLO).unwrap();
        let second = Transcript::from_json(HELLO).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.name, "hello");
        assert_eq!(first.frames.first().unwrap().event, EventName::Metadata);
        assert!(
            first
                .frames
                .iter()
                .any(|frame| frame.event == EventName::MessagesPartial)
        );
        assert_eq!(
            first.frames.last().unwrap().event,
            EventName::Values,
            "the final snapshot is what the transcript replays"
        );
    }

    #[test]
    fn sse_parsing_joins_multiline_data_and_crlf() {
        let input = "event: values\r\ndata: {\"a\":\r\ndata: 1}\r\n\r\n";
        let frames = parse_sse(input).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].event, "values");
        assert_eq!(frames[0].data["a"], 1);
    }

    #[test]
    fn frame_without_data_is_malformed() {
        let error = parse_sse("event: values\n\n").unwrap_err();
        assert!(matches!(error, ProtoError::MalformedFrame(_)));
    }

    #[test]
    fn message_content_joins_content_blocks() {
        let message = serde_json::json!({
            "content": [{"type": "text", "text": "Hello"}, {"type": "text", "text": " there"}],
            "type": "ai"
        });
        assert_eq!(message_content(&message).as_deref(), Some("Hello there"));
        assert_eq!(message_role(&message), "ai");
    }
}
