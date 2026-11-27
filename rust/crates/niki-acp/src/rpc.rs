//! JSON-RPC 2.0 over newline-delimited JSON.
//!
//! The protocol's reference server frames every message as one compact JSON
//! object on its own line and flushes it, with no `Content-Length` header and
//! no batch arrays. A frame is a request when it carries `method` and `id`, a
//! notification when it carries `method` alone, and a response when it carries
//! `id` with `result` or `error`. Responses omit absent optional members, and
//! an error object always carries `code`, `message` and `data`.

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Map, Value, json};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};

/// A frame that is not valid JSON is dropped, as the reference server does
/// (it logs the line and keeps reading), so this server never sends the
/// `-32700` parse error.
/// A frame that is not a request, notification or response.
pub(crate) const INVALID_REQUEST: i64 = -32600;
/// A method this server does not implement.
pub(crate) const METHOD_NOT_FOUND: i64 = -32601;
/// A request whose params do not match the method's shape.
pub(crate) const INVALID_PARAMS: i64 = -32602;
/// A failure inside the server.
pub(crate) const INTERNAL_ERROR: i64 = -32603;
/// A session the server does not hold.
pub(crate) const RESOURCE_NOT_FOUND: i64 = -32002;

/// One JSON-RPC error, in the shape the protocol's clients decode.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RpcError {
    code: i64,
    message: String,
    data: Option<Value>,
}

impl RpcError {
    pub(crate) fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub(crate) fn with_data(code: i64, message: impl Into<String>, data: Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }

    pub(crate) fn method_not_found(method: &str) -> Self {
        Self::with_data(
            METHOD_NOT_FOUND,
            "Method not found",
            json!({ "method": method }),
        )
    }

    pub(crate) fn invalid_params(details: impl Into<String>) -> Self {
        Self::with_data(
            INVALID_PARAMS,
            "Invalid params",
            json!({ "details": details.into() }),
        )
    }

    pub(crate) fn invalid_params_message(message: impl Into<String>) -> Self {
        Self::new(INVALID_PARAMS, message)
    }

    pub(crate) fn invalid_params_field(field: &str, reason: impl Into<String>) -> Self {
        Self::with_data(
            INVALID_PARAMS,
            "Invalid params",
            json!({ field: reason.into() }),
        )
    }

    pub(crate) fn internal(details: impl Into<String>) -> Self {
        Self::with_data(
            INTERNAL_ERROR,
            "Internal error",
            json!({ "details": details.into() }),
        )
    }

    pub(crate) fn resource_not_found(session_id: &str) -> Self {
        Self::with_data(
            RESOURCE_NOT_FOUND,
            "Resource not found",
            json!({ "uri": session_id }),
        )
    }

    pub(crate) fn to_value(&self) -> Value {
        json!({
            "code": self.code,
            "message": self.message,
            "data": self.data.clone().unwrap_or(Value::Null),
        })
    }

    fn from_value(value: Value) -> Self {
        Self {
            code: value
                .get("code")
                .and_then(Value::as_i64)
                .unwrap_or(INTERNAL_ERROR),
            message: value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Error")
                .to_owned(),
            data: value.get("data").cloned(),
        }
    }
}

/// What one incoming line means.
pub(crate) enum Incoming {
    /// `method` and `id`: answer it.
    Request {
        id: Value,
        /// `None` when `method` was present but not a string.
        method: Option<String>,
        params: Value,
    },
    /// `method` alone: apply it, answer nothing.
    Notification { method: String, params: Value },
    /// `id` with `result` or `error`: settle our own request.
    Response {
        id: Value,
        outcome: Result<Value, RpcError>,
    },
    /// Neither shape, so there is nothing to do.
    Ignore,
}

pub(crate) fn classify(frame: Value) -> Incoming {
    let Value::Object(mut object) = frame else {
        return Incoming::Ignore;
    };
    let method = object.remove("method");
    let id = object.remove("id");
    let params = object.remove("params").unwrap_or(Value::Null);
    match (method, id) {
        (Some(Value::String(method)), Some(id)) => Incoming::Request {
            id,
            method: Some(method),
            params,
        },
        (Some(_), Some(id)) => Incoming::Request {
            id,
            method: None,
            params,
        },
        (Some(Value::String(method)), None) => Incoming::Notification { method, params },
        (Some(_), None) => Incoming::Ignore,
        (None, Some(id)) => Incoming::Response {
            id,
            outcome: response_outcome(object),
        },
        (None, None) => Incoming::Ignore,
    }
}

fn response_outcome(mut object: Map<String, Value>) -> Result<Value, RpcError> {
    if let Some(result) = object.remove("result") {
        return Ok(result);
    }
    if let Some(error) = object.remove("error") {
        return Err(RpcError::from_value(error));
    }
    Ok(Value::Null)
}

/// The client's answer to one request the server sent.
type Answer = Result<Value, RpcError>;

/// Requests awaiting an answer, keyed by the id they were sent with.
type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Answer>>>>;

/// The outgoing side of one connection.
///
/// Frames go through a queue so a handler can notify the client while it
/// awaits, and a request the server itself sends — a permission prompt — is
/// awaited by id. The reader resolves those ids when the client answers.
#[derive(Clone)]
pub(crate) struct Connection {
    outgoing: mpsc::UnboundedSender<Value>,
    pending: Pending,
    next_id: Arc<AtomicU64>,
}

impl Connection {
    pub(crate) fn channel() -> (Self, mpsc::UnboundedReceiver<Value>) {
        let (outgoing, frames) = mpsc::unbounded_channel();
        let connection = Self {
            outgoing,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(0)),
        };
        (connection, frames)
    }

    fn send(&self, frame: Value) {
        // A closed queue means the writer task is gone; the reader will see the
        // same closed transport and end the session.
        let _ = self.outgoing.send(frame);
    }

    pub(crate) fn notify(&self, method: &str, params: Value) {
        self.send(json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }));
    }

    pub(crate) fn respond(&self, id: Value, outcome: Result<Value, RpcError>) {
        let mut frame = json!({ "jsonrpc": "2.0", "id": id });
        match outcome {
            Ok(result) => frame["result"] = result,
            Err(error) => frame["error"] = error.to_value(),
        }
        self.send(frame);
    }

    /// Send one request and wait for the client's answer.
    pub(crate) async fn request(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = oneshot::channel();
        self.lock_pending().insert(id, sender);
        self.send(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }));
        match receiver.await {
            Ok(outcome) => outcome,
            Err(_) => Err(RpcError::new(INTERNAL_ERROR, "Connection closed")),
        }
    }

    pub(crate) fn resolve(&self, id: &Value, outcome: Result<Value, RpcError>) {
        let Some(id) = id.as_u64() else {
            return;
        };
        let sender = self.lock_pending().remove(&id);
        if let Some(sender) = sender {
            let _ = sender.send(outcome);
        }
    }

    /// Fail every request the server is still waiting on.
    pub(crate) fn shutdown(&self) {
        let pending: Vec<_> = self
            .lock_pending()
            .drain()
            .map(|(_, sender)| sender)
            .collect();
        for sender in pending {
            let _ = sender.send(Err(RpcError::new(INTERNAL_ERROR, "Connection closed")));
        }
    }

    fn lock_pending(&self) -> std::sync::MutexGuard<'_, HashMap<u64, oneshot::Sender<Answer>>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Write each queued frame as one line and flush it.
pub(crate) async fn write_frames<W>(
    mut writer: W,
    mut frames: mpsc::UnboundedReceiver<Value>,
) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    while let Some(frame) = frames.recv().await {
        let mut line = serde_json::to_vec(&frame).map_err(io::Error::other)?;
        line.push(b'\n');
        writer.write_all(&line).await?;
        writer.flush().await?;
    }
    Ok(())
}

/// Read the next JSON frame, skipping blank and malformed lines.
pub(crate) async fn read_frame<R>(reader: &mut R) -> io::Result<Option<Value>>
where
    R: AsyncBufRead + Unpin,
{
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            return Ok(None);
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str(line) {
            Ok(frame) => return Ok(Some(frame)),
            Err(_) => continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_notification_and_response_are_told_apart() {
        let request = classify(json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "session/prompt",
            "params": { "sessionId": "s" },
        }));
        let Incoming::Request { id, method, params } = request else {
            panic!("not a request");
        };
        assert_eq!(id, json!(7));
        assert_eq!(method.as_deref(), Some("session/prompt"));
        assert_eq!(params, json!({ "sessionId": "s" }));

        let notification = classify(json!({
            "jsonrpc": "2.0",
            "method": "session/cancel",
            "params": { "sessionId": "s" },
        }));
        let Incoming::Notification { method, .. } = notification else {
            panic!("not a notification");
        };
        assert_eq!(method, "session/cancel");

        let response = classify(json!({ "jsonrpc": "2.0", "id": 3, "result": { "ok": true } }));
        let Incoming::Response { id, outcome } = response else {
            panic!("not a response");
        };
        assert_eq!(id, json!(3));
        assert_eq!(outcome.expect("result"), json!({ "ok": true }));
    }

    #[test]
    fn an_error_object_carries_code_message_and_data() {
        let error = RpcError::method_not_found("session/delete").to_value();
        assert_eq!(error["code"], json!(METHOD_NOT_FOUND));
        assert_eq!(error["message"], json!("Method not found"));
        assert_eq!(error["data"], json!({ "method": "session/delete" }));
    }

    #[test]
    fn a_client_error_response_becomes_an_rpc_error() {
        let frame = classify(json!({
            "jsonrpc": "2.0",
            "id": 11,
            "error": { "code": -32602, "message": "Invalid params", "data": { "details": "bad" } },
        }));
        let Incoming::Response { outcome, .. } = frame else {
            panic!("not a response");
        };
        let error = outcome.expect_err("error");
        assert_eq!(error.code, INVALID_PARAMS);
        assert_eq!(error.message, "Invalid params");
        assert_eq!(error.data, Some(json!({ "details": "bad" })));
    }

    #[tokio::test]
    async fn frames_are_one_json_object_per_line_and_bad_lines_are_skipped() {
        let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"a\"}\n\nnot json\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"b\"}\n";
        let mut reader = tokio::io::BufReader::new(&input[..]);
        let first = read_frame(&mut reader).await.expect("read").expect("frame");
        assert_eq!(first["method"], json!("a"));
        let second = read_frame(&mut reader).await.expect("read").expect("frame");
        assert_eq!(second["method"], json!("b"));
        assert!(read_frame(&mut reader).await.expect("read").is_none());
    }
}
