//! HTTP/SSE client for the Niki engine.
//!
//! Owns the connection lifecycle — thread creation, a streaming run, cancel,
//! and state reads — and decodes SSE incrementally. Rendering never happens
//! here.

use std::pin::Pin;

use futures_util::stream::{Stream, StreamExt};
use niki_proto::{EventName, Frame, ProtoError, RawFrame, parse_sse};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A run request in the shape the engine accepts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunRequest {
    pub assistant_id: String,
    pub input: Value,
    #[serde(default)]
    pub stream_mode: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
}

impl RunRequest {
    /// A prompt run with the modes the Niki client needs.
    #[must_use]
    pub fn prompt(assistant: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            assistant_id: assistant.into(),
            input: serde_json::json!({"messages": [{"type": "human", "content": text.into()}]}),
            stream_mode: vec![
                "messages".to_owned(),
                "updates".to_owned(),
                "values".to_owned(),
            ],
            config: None,
        }
    }
}

/// Incremental SSE decoder over raw bytes.
///
/// Feed arbitrary byte-aligned chunks; complete frames come back as soon as a
/// blank line closes them. Buffering bytes rather than text keeps a multi-byte
/// character that straddles two network chunks intact.
#[derive(Debug, Default)]
pub struct SseDecoder {
    buffer: Vec<u8>,
}

impl SseDecoder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a text chunk and return every frame it completes.
    ///
    /// # Errors
    ///
    /// Returns the parse error for a malformed completed frame.
    pub fn push_str(&mut self, chunk: &str) -> Result<Vec<RawFrame>, ProtoError> {
        self.push(chunk.as_bytes())
    }

    /// Append a byte chunk and return every frame it completes.
    ///
    /// # Errors
    ///
    /// Returns the parse error for a malformed completed frame or a frame that
    /// is not valid UTF-8.
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<RawFrame>, ProtoError> {
        self.buffer.extend_from_slice(chunk);
        let mut frames = Vec::new();
        while let Some((end, separator)) = block_end(&self.buffer) {
            let block: Vec<u8> = self.buffer.drain(..end + separator).collect();
            let text = decode(&block[..end])?;
            if text.trim().is_empty() {
                continue;
            }
            frames.extend(parse_sse(&format!("{text}\n\n"))?);
        }
        Ok(frames)
    }

    /// Flush a final frame that arrived without a trailing blank line.
    ///
    /// # Errors
    ///
    /// Returns the parse error for a malformed trailing frame.
    pub fn finish(&mut self) -> Result<Vec<RawFrame>, ProtoError> {
        let block = std::mem::take(&mut self.buffer);
        let text = decode(&block)?;
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }
        parse_sse(&format!("{text}\n\n"))
    }
}

fn decode(bytes: &[u8]) -> Result<String, ProtoError> {
    String::from_utf8(bytes.to_vec()).map_err(|error| ProtoError::Json(error.to_string()))
}

fn block_end(buffer: &[u8]) -> Option<(usize, usize)> {
    let lf = buffer
        .windows(2)
        .position(|pair| pair == b"\n\n")
        .map(|i| (i, 2));
    let crlf = buffer
        .windows(4)
        .position(|quad| quad == b"\r\n\r\n")
        .map(|i| (i, 4));
    match (lf, crlf) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (Some(found), None) | (None, Some(found)) => Some(found),
        (None, None) => None,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("protocol error: {0}")]
    Proto(#[from] ProtoError),
    #[error("engine returned {status}: {body}")]
    Status { status: u16, body: String },
}

/// A connection to the Niki engine.
#[derive(Clone)]
pub struct EngineClient {
    base: String,
    http: reqwest::Client,
}

impl EngineClient {
    #[must_use]
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::new(),
        }
    }

    /// Create a thread and return its id.
    ///
    /// # Errors
    ///
    /// Returns an error for transport failures or a response without a
    /// `thread_id`.
    pub async fn create_thread(&self) -> Result<String, ClientError> {
        let response = self
            .http
            .post(format!("{}/threads", self.base))
            .json(&serde_json::json!({}))
            .send()
            .await?;
        let status = response.status();
        let body: Value = response.json().await?;
        if !status.is_success() {
            return Err(ClientError::Status {
                status: status.as_u16(),
                body: body.to_string(),
            });
        }
        body.get("thread_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| ClientError::Status {
                status: status.as_u16(),
                body: format!("missing thread_id: {body}"),
            })
    }

    /// Start a run and stream its frames.
    ///
    /// # Errors
    ///
    /// Returns an error when the connection or the initial response fails. Per
    /// frame errors surface on the returned stream.
    pub async fn stream_run(
        &self,
        thread_id: &str,
        request: &RunRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Frame, ClientError>> + Send>>, ClientError> {
        let response = self
            .http
            .post(format!("{}/threads/{thread_id}/runs/stream", self.base))
            .json(request)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ClientError::Status {
                status: status.as_u16(),
                body,
            });
        }

        let mut decoder = SseDecoder::new();
        let frames = response.bytes_stream().flat_map(move |chunk| {
            let result = match chunk {
                Ok(bytes) => decoder.push(&bytes),
                Err(error) => Err(ProtoError::Json(error.to_string())),
            };
            let frames = match result {
                Ok(raw) => raw.into_iter().map(typed).collect::<Vec<_>>(),
                Err(error) => vec![Err(ClientError::Proto(error))],
            };
            futures_util::stream::iter(frames)
        });
        Ok(Box::pin(frames))
    }

    /// Ask the engine to cancel every active run on a thread.
    ///
    /// # Errors
    ///
    /// Returns an error for transport failures or a non-success response.
    pub async fn cancel(&self, thread_id: &str) -> Result<(), ClientError> {
        let response = self
            .http
            .post(format!("{}/threads/{thread_id}/runs/cancel", self.base))
            .json(&serde_json::json!({"action": "interrupt"}))
            .send()
            .await?;
        if response.status().is_success() {
            return Ok(());
        }
        Err(ClientError::Status {
            status: response.status().as_u16(),
            body: response.text().await.unwrap_or_default(),
        })
    }

    /// Read the latest state snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for transport failures or a non-success response.
    pub async fn state(&self, thread_id: &str) -> Result<Value, ClientError> {
        self.get_json(&format!("{}/threads/{thread_id}/state", self.base))
            .await
    }

    async fn get_json(&self, url: &str) -> Result<Value, ClientError> {
        let response = self.http.get(url).send().await.map_err(ClientError::Http)?;
        let status = response.status();
        let body: Value = response.json().await?;
        if status.is_success() {
            Ok(body)
        } else {
            Err(ClientError::Status {
                status: status.as_u16(),
                body: body.to_string(),
            })
        }
    }
}

fn typed(frame: RawFrame) -> Result<Frame, ClientError> {
    Ok(Frame {
        event: EventName::from_wire(&frame.event)?,
        data: frame.data,
    })
}

/// Join a stream into a transcript, stopping at the error frame.
///
/// # Errors
///
/// Returns the first transport or protocol error.
pub async fn collect_transcript(
    mut stream: Pin<Box<dyn Stream<Item = Result<Frame, ClientError>> + Send>>,
) -> Result<Vec<Frame>, ClientError> {
    let mut frames = Vec::new();
    while let Some(frame) = stream.next().await {
        let frame = frame?;
        let terminal = frame.event == EventName::Error;
        frames.push(frame);
        if terminal {
            break;
        }
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: &str = "event: metadata\r\ndata: {\"run_id\":\"r1\"}\r\n\r\nevent: values\r\ndata: {\"messages\":[]}\r\n\r\n";

    #[test]
    fn decoder_handles_every_split_point() {
        let expected = parse_sse(RAW).unwrap();
        for split in 0..=RAW.len() {
            if !RAW.is_char_boundary(split) {
                continue;
            }
            let mut decoder = SseDecoder::new();
            let mut frames = decoder.push_str(&RAW[..split]).unwrap();
            frames.extend(decoder.push_str(&RAW[split..]).unwrap());
            frames.extend(decoder.finish().unwrap());
            assert_eq!(frames, expected, "split at {split}");
        }
    }

    #[test]
    fn decoder_handles_lf_only_and_trailing_frame() {
        let mut decoder = SseDecoder::new();
        assert!(
            decoder
                .push_str("event: end\ndata: {}\n")
                .unwrap()
                .is_empty()
        );
        let frame = decoder.finish().unwrap().pop().unwrap();
        assert_eq!(frame.event, "end");
    }

    #[test]
    fn a_multibyte_character_split_across_chunks_survives() {
        let text = "event: messages/partial\ndata: {\"text\":\"héllo\"}\n\n";
        let bytes = text.as_bytes();
        let boundary = text.find('é').unwrap() + 1;
        let mut decoder = SseDecoder::new();
        let mut frames = decoder.push(&bytes[..boundary]).unwrap();
        frames.extend(decoder.push(&bytes[boundary..]).unwrap());
        frames.extend(decoder.finish().unwrap());
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data["text"], "héllo");
    }

    #[test]
    fn request_matches_the_engine_shape() {
        let request = RunRequest::prompt("agent", "Say hello");
        let value = serde_json::to_value(&request).unwrap();
        assert_eq!(value["assistant_id"], "agent");
        assert_eq!(value["input"]["messages"][0]["type"], "human");
        assert_eq!(value["stream_mode"][0], "messages");
    }

    #[test]
    fn unknown_event_fails_the_stream_mapping() {
        let frame = RawFrame {
            event: "messages/delta".to_owned(),
            data: Value::Null,
            id: None,
        };
        let error = typed(frame).unwrap_err();
        assert!(matches!(
            error,
            ClientError::Proto(ProtoError::UnknownEvent(_))
        ));
    }
}
