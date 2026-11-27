//! Server-sent events: framing and the shared request path.
//!
//! Both wire formats are SSE with a different payload shape. The decoder here
//! only does framing; each adapter owns its payload.

use std::pin::Pin;
use std::time::Duration;

use futures_util::Stream;
use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use reqwest::Response;
use reqwest::header::RETRY_AFTER;
use reqwest::redirect::Policy;
use tokio::time::sleep;

use crate::error::ProviderError;
use crate::retry::{self, Failure};
use crate::{RetryPolicy, StreamEvent};

/// One `event:`/`data:` pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// Value of the `event:` line, when the stream sets one.
    pub event: Option<String>,
    /// Joined `data:` lines.
    pub data: String,
}

/// Incremental SSE frame decoder.
///
/// Text arrives in chunks that rarely align with frame boundaries, so bytes are
/// buffered until a blank line closes a frame.
#[derive(Debug, Default)]
pub struct SseDecoder {
    buffer: String,
}

impl SseDecoder {
    /// Add a chunk of text.
    pub fn push(&mut self, chunk: &str) {
        self.buffer.push_str(chunk);
    }

    /// Take the next complete frame, if one is buffered.
    pub fn next_event(&mut self) -> Option<SseEvent> {
        loop {
            let boundary = self
                .buffer
                .find("\n\n")
                .or_else(|| self.buffer.find("\r\n\r\n"))?;
            let width = if self.buffer[boundary..].starts_with("\r\n\r\n") {
                4
            } else {
                2
            };
            let frame: String = self.buffer.drain(..boundary + width).collect();

            let mut event = None;
            let mut data = String::new();
            for line in frame.lines() {
                let line = line.trim_end_matches('\r');
                if let Some(value) = line.strip_prefix("event:") {
                    event = Some(value.trim().to_owned());
                } else if let Some(value) = line.strip_prefix("data:") {
                    if !data.is_empty() {
                        data.push('\n');
                    }
                    data.push_str(value.strip_prefix(' ').unwrap_or(value));
                }
            }
            if event.is_none() && data.is_empty() {
                continue;
            }
            return Some(SseEvent { event, data });
        }
    }
}

/// An HTTP client configured for long-lived streams.
///
/// # Errors
///
/// Returns [`ProviderError::Transport`] when the client cannot be built.
pub(crate) fn client(provider: &str) -> Result<reqwest::Client, ProviderError> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .redirect(Policy::none())
        .build()
        .map_err(|e| ProviderError::Transport {
            provider: provider.to_owned(),
            detail: e.to_string(),
        })
}

/// Send a request, retrying a transient failure before the body is consumed.
///
/// `make` builds the request again for each attempt because a `RequestBuilder`
/// cannot be cloned. Only the response status and headers are inspected here;
/// the body is streamed by [`stream_events`] and never retried, because a
/// retry after the first event would duplicate text.
///
/// # Errors
///
/// Returns [`ProviderError::Http`] for the last non-success status and
/// [`ProviderError::Transport`] when every attempt failed to connect.
pub(crate) async fn send_with_retries<F>(
    provider: &str,
    policy: RetryPolicy,
    mut make: F,
) -> Result<Response, ProviderError>
where
    F: FnMut() -> Result<reqwest::RequestBuilder, ProviderError>,
{
    retry::with_retries(
        policy,
        move || {
            let request = make();
            async move {
                let response = request.map_err(Failure::fatal)?.send().await.map_err(|e| {
                    Failure::fatal(ProviderError::Transport {
                        provider: provider.to_owned(),
                        detail: e.to_string(),
                    })
                })?;
                let status = response.status().as_u16();
                if response.status().is_success() {
                    return Ok(response);
                }
                let retry_after = response
                    .headers()
                    .get(RETRY_AFTER)
                    .and_then(|value| value.to_str().ok())
                    .and_then(retry::retry_after_ms);
                let body = response.text().await.unwrap_or_default();
                Err(Failure::new(
                    ProviderError::Http {
                        provider: provider.to_owned(),
                        status,
                        body: body.chars().take(400).collect(),
                    },
                    retry_after,
                ))
            }
        },
        sleep,
    )
    .await
}

type Body = Pin<Box<dyn Stream<Item = Result<Vec<u8>, reqwest::Error>> + Send>>;
type Parser = Box<dyn FnMut(SseEvent) -> Result<Vec<StreamEvent>, ProviderError> + Send>;

struct Pump {
    provider: String,
    body: Body,
    decoder: SseDecoder,
    parse: Parser,
    pending: std::collections::VecDeque<StreamEvent>,
    done: bool,
}

/// Read `response` as SSE, mapping each complete frame with `parse`.
///
/// A frame whose payload is `[DONE]` ends the stream, as does a
/// [`StreamEvent::Done`].
pub(crate) fn stream_events<P>(
    provider: impl Into<String>,
    response: Response,
    parse: P,
) -> BoxStream<'static, Result<StreamEvent, ProviderError>>
where
    P: FnMut(SseEvent) -> Result<Vec<StreamEvent>, ProviderError> + Send + 'static,
{
    let pump = Pump {
        provider: provider.into(),
        body: Box::pin(
            response
                .bytes_stream()
                .map(|chunk| chunk.map(|b| b.to_vec())),
        ),
        decoder: SseDecoder::default(),
        parse: Box::new(parse),
        pending: std::collections::VecDeque::new(),
        done: false,
    };
    Box::pin(futures_util::stream::unfold(
        Some(pump),
        |state| async move {
            let mut pump = state?;
            if pump.done {
                return None;
            }
            loop {
                if let Some(next) = pump.pending.pop_front() {
                    // A `Done` reason is not the end of the stream: OpenAI
                    // sends the usage chunk after it and terminates with
                    // `[DONE]`. Ending here would lose the token counts.
                    return Some((Ok(next), Some(pump)));
                }
                while let Some(event) = pump.decoder.next_event() {
                    if event.data.trim() == "[DONE]" {
                        pump.done = true;
                        return None;
                    }
                    match (pump.parse)(event) {
                        Ok(parsed) => {
                            if parsed.is_empty() {
                                continue;
                            }
                            pump.pending.extend(parsed);
                            // Emit before touching the socket again.
                            break;
                        }
                        Err(e) => {
                            pump.done = true;
                            return Some((Err(e), Some(pump)));
                        }
                    }
                }
                if !pump.pending.is_empty() {
                    continue;
                }
                match pump.body.next().await {
                    Some(Ok(chunk)) => pump.decoder.push(&String::from_utf8_lossy(&chunk)),
                    Some(Err(e)) => {
                        pump.done = true;
                        return Some((
                            Err(ProviderError::Transport {
                                provider: pump.provider.clone(),
                                detail: e.to_string(),
                            }),
                            Some(pump),
                        ));
                    }
                    None => {
                        pump.done = true;
                        return None;
                    }
                }
            }
        },
    ))
}
