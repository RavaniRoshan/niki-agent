//! Provider adapters against a std-only mock server: no test reaches a live
//! model provider.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::thread;

use futures_util::StreamExt;
use niki_provider::error::ProviderError;
use niki_provider::openai::ToolCallAssembler;
use niki_provider::{
    AnthropicProvider, ChatRequest, Message, OpenAiProvider, Provider, RetryPolicy, StopReason,
    StreamEvent, ToolSpec, Usage,
};

struct MockResponse {
    status: u16,
    body: String,
    content_type: &'static str,
}

impl MockResponse {
    fn sse(body: &str) -> Self {
        Self {
            status: 200,
            body: body.to_owned(),
            content_type: "text/event-stream",
        }
    }

    fn status(status: u16, body: &str) -> Self {
        Self {
            status,
            body: body.to_owned(),
            content_type: "application/json",
        }
    }
}

/// Serve each response to one connection, in order, then close the listener.
struct MockServer {
    addr: SocketAddr,
    handle: thread::JoinHandle<Vec<String>>,
}

impl MockServer {
    fn start(responses: Vec<MockResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let Ok((stream, _)) = listener.accept() else {
                    break;
                };
                let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                let mut head = String::new();
                let mut length = 0usize;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap_or(0);
                    }
                    head.push_str(&line);
                    if line == "\r\n" || line == "\n" {
                        break;
                    }
                }
                let mut body = vec![0u8; length];
                let _ = reader.read_exact(&mut body);
                requests.push(format!("{head}\n{}", String::from_utf8_lossy(&body)));

                let mut stream = stream;
                let response_head = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    response.status,
                    reason(response.status),
                    response.content_type,
                    response.body.len()
                );
                let _ = stream.write_all(response_head.as_bytes());
                let _ = stream.write_all(response.body.as_bytes());
                let _ = stream.flush();
            }
            requests
        });
        Self { addr, handle }
    }

    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn requests(self) -> Vec<String> {
        self.handle.join().expect("server thread")
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        429 => "Too Many Requests",
        _ => "Internal Server Error",
    }
}

fn fast_retry() -> RetryPolicy {
    RetryPolicy {
        attempts: 2,
        base_delay_ms: 1,
    }
}

fn request() -> ChatRequest {
    ChatRequest {
        model: "test-model".to_owned(),
        system: "be brief".to_owned(),
        messages: vec![Message::user("hi")],
        tools: vec![ToolSpec {
            name: "read_file".to_owned(),
            description: "read a file".to_owned(),
            schema: serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        }],
        max_tokens: 64,
    }
}

async fn collect(
    stream: impl futures_util::Stream<Item = Result<StreamEvent, ProviderError>>,
) -> Vec<StreamEvent> {
    let events: Vec<_> = stream.collect().await;
    events
        .into_iter()
        .map(|event| event.expect("no provider error"))
        .collect()
}

#[tokio::test]
async fn openai_streams_text_then_done() {
    let body = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\" world\"}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":2}}\n\n",
        "data: [DONE]\n\n",
    );
    let server = MockServer::start(vec![MockResponse::sse(body)]);
    let provider = OpenAiProvider::new("mock", server.url(), "k", RetryPolicy::default_policy())
        .expect("provider");

    let events = collect(provider.stream(request())).await;

    assert_eq!(
        events,
        vec![
            StreamEvent::TextDelta("Hello".to_owned()),
            StreamEvent::TextDelta(" world".to_owned()),
            StreamEvent::Done(StopReason::EndTurn),
            // The usage chunk follows the finish reason; ending the stream at
            // `Done` would drop it.
            StreamEvent::Usage(Usage {
                input_tokens: 7,
                output_tokens: 2,
            }),
        ]
    );
    let requests = server.requests();
    assert!(requests[0].contains("POST /chat/completions"));
    assert!(requests[0].contains("\"stream_options\":{\"include_usage\":true}"));
}

#[tokio::test]
async fn openai_assembles_tool_call_fragments() {
    let body = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"read_file\",\"arguments\":\"{\\\"pa\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"th\\\": \\\"a.txt\\\"}\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
    );
    let server = MockServer::start(vec![MockResponse::sse(body)]);
    let provider = OpenAiProvider::new("mock", server.url(), "k", RetryPolicy::default_policy())
        .expect("provider");

    let mut assembler = ToolCallAssembler::default();
    let mut done = None;
    let mut stream = provider.stream(request());
    while let Some(event) = stream.next().await {
        match event.expect("no provider error") {
            StreamEvent::ToolCallDelta {
                index,
                id,
                name,
                arguments,
            } => assembler.push(index, id, name, &arguments),
            StreamEvent::Done(reason) => done = Some(reason),
            _ => {}
        }
    }
    let _ = server.requests();

    assert_eq!(done, Some(StopReason::ToolUse));
    let calls = assembler.finish();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id, "call_1");
    assert_eq!(calls[0].name, "read_file");
    let parsed: serde_json::Value = serde_json::from_str(&calls[0].arguments).expect("valid json");
    assert_eq!(parsed["path"], "a.txt");
}

#[tokio::test]
async fn openai_retries_a_429_then_succeeds() {
    let body = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"},\"finish_reason\":\"stop\"}]}\n\n";
    let server = MockServer::start(vec![
        MockResponse::status(429, "{\"error\":\"slow down\"}"),
        MockResponse::sse(body),
    ]);
    let provider = OpenAiProvider::new("mock", server.url(), "k", fast_retry()).expect("provider");

    let events = collect(provider.stream(request())).await;
    let requests = server.requests();

    assert!(matches!(events[0], StreamEvent::TextDelta(_)));
    assert!(matches!(events.last(), Some(StreamEvent::Done(_))));
    assert_eq!(requests.len(), 2, "one retry means two requests");
}

#[tokio::test]
async fn openai_reports_a_400_with_provider_and_status() {
    let server = MockServer::start(vec![MockResponse::status(400, "{\"error\":\"bad model\"}")]);
    let provider = OpenAiProvider::new("groq", server.url(), "k", RetryPolicy::default_policy())
        .expect("provider");

    let events: Vec<_> = provider.stream(request()).collect().await;
    let _ = server.requests();

    match &events[0] {
        Err(ProviderError::Http {
            provider,
            status,
            body,
        }) => {
            assert_eq!(provider, "groq");
            assert_eq!(*status, 400);
            assert!(body.contains("bad model"));
        }
        other => panic!("expected an HTTP error, got {other:?}"),
    }
    assert_eq!(events.len(), 1, "a permanent failure is not retried");
}

#[tokio::test]
async fn anthropic_streams_text_tool_use_and_usage() {
    let body = concat!(
        "event: message_start\n",
        "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":11,\"output_tokens\":0}}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Let me \"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"look.\"}}\n\n",
        "event: content_block_start\n",
        "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_1\",\"name\":\"read_file\",\"input\":{}}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\" \\\"a.txt\\\"}\"}}\n\n",
        "event: message_delta\n",
        "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":9}}\n\n",
        "event: message_stop\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );
    let server = MockServer::start(vec![MockResponse::sse(body)]);
    let provider = AnthropicProvider::new(
        "anthropic",
        server.url(),
        "k",
        RetryPolicy::default_policy(),
    )
    .expect("provider");

    let events = collect(provider.stream(request())).await;
    let requests = server.requests();

    assert_eq!(events[0], StreamEvent::TextDelta("Let me ".to_owned()));
    assert_eq!(events[1], StreamEvent::TextDelta("look.".to_owned()));
    assert_eq!(
        events[2],
        StreamEvent::ToolCallDelta {
            index: 1,
            id: Some("toolu_1".to_owned()),
            name: Some("read_file".to_owned()),
            arguments: String::new(),
        }
    );
    let mut assembler = ToolCallAssembler::default();
    for event in &events {
        if let StreamEvent::ToolCallDelta {
            index,
            id,
            name,
            arguments,
        } = event
        {
            assembler.push(*index, id.clone(), name.clone(), arguments);
        }
    }
    let calls = assembler.finish();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read_file");
    assert_eq!(calls[0].arguments, "{\"path\": \"a.txt\"}");
    assert!(events.contains(&StreamEvent::Usage(niki_provider::Usage {
        input_tokens: 11,
        output_tokens: 9,
    })));
    assert_eq!(events.last(), Some(&StreamEvent::Done(StopReason::ToolUse)));
    assert!(requests[0].contains("POST /v1/messages"));
    assert!(requests[0].contains("anthropic-version: 2023-06-01"));
}

#[tokio::test]
async fn sse_decoder_handles_frames_split_across_chunks() {
    use niki_provider::sse::SseDecoder;

    let mut decoder = SseDecoder::default();
    decoder.push("event: a\ndata: {\"x\":1}\n\ndata: sec");
    assert_eq!(
        decoder.next_event(),
        Some(niki_provider::sse::SseEvent {
            event: Some("a".to_owned()),
            data: "{\"x\":1}".to_owned(),
        })
    );
    assert_eq!(decoder.next_event(), None);
    decoder.push("ond\n\n");
    assert_eq!(
        decoder.next_event(),
        Some(niki_provider::sse::SseEvent {
            event: None,
            data: "second".to_owned(),
        })
    );
}
