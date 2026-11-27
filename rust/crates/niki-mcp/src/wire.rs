use std::collections::BTreeMap;
use std::io;
use std::process::Stdio as ProcessStdio;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::Mutex;

use crate::tool::parse_tools;
use crate::{CallResult, McpError, McpTool, ToolBridge, Transport};

/// The MCP protocol revision this client advertises.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// How long one request may wait for its response by default.
///
/// This bounds a dead server without rejecting a tool that legitimately takes
/// a while; a host that needs another budget sets the per-server `timeout`
/// key or calls [`Server::connect_with`](crate::Server::connect_with).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// How long a server may take to answer its handshake by default.
///
/// A server that accepts the process or connection and then never answers is
/// abandoned at this point: startup must not wait a minute for one quiet
/// server. The host can override it with `NIKI_MCP_STARTUP_TIMEOUT_SECS` and
/// [`load_with`](crate::load_with).
pub const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

/// One connected MCP server.
///
/// [`Server::connect`] performs the handshake and `tools/list` once; later
/// calls share the same child process (stdio) or endpoint (HTTP). Every request
/// is serialised behind one mutex because a line-delimited stream cannot
/// multiplex responses safely.
pub struct Server {
    name: String,
    protocol_version: String,
    tools: Vec<McpTool>,
    connection: Mutex<Connection>,
}

impl Server {
    /// Connect with the default timeout and list the server's tools.
    ///
    /// # Errors
    ///
    /// Returns [`McpError`] when the process or endpoint cannot be reached, the
    /// handshake fails, or `tools/list` does not decode.
    pub async fn connect(name: &str, transport: Transport) -> Result<Self, McpError> {
        Self::connect_with(name, transport, DEFAULT_TIMEOUT).await
    }

    /// Connect with an explicit request timeout.
    ///
    /// # Errors
    ///
    /// See [`Server::connect`].
    pub async fn connect_with(
        name: &str,
        transport: Transport,
        timeout: Duration,
    ) -> Result<Self, McpError> {
        let mut connection = Connection::open(name, transport, timeout).await?;
        let initialized = connection
            .request(
                "initialize",
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {
                        "name": "niki",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                }),
            )
            .await?;
        let protocol_version = initialized
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or(PROTOCOL_VERSION)
            .to_owned();
        connection
            .notify("notifications/initialized", json!({}))
            .await?;
        let listed = connection.request("tools/list", json!({})).await?;
        let tools = parse_tools(name, &listed)?;
        Ok(Self {
            name: name.to_owned(),
            protocol_version,
            tools,
            connection: Mutex::new(connection),
        })
    }

    /// Connect, but give the whole handshake at most `startup`.
    ///
    /// The setup calls still use [`DEFAULT_TIMEOUT`] each; this budget covers
    /// them together, so a server that is silent from the first byte costs one
    /// startup budget and is then dropped. Dropping the future also drops the
    /// stdio child, which `kill_on_drop` reaps.
    ///
    /// # Errors
    ///
    /// Returns [`McpError::Timeout`] naming `initialize` when the budget
    /// elapses, and every error [`Server::connect`] can return.
    pub async fn connect_startup(
        name: &str,
        transport: Transport,
        startup: Duration,
    ) -> Result<Self, McpError> {
        Self::connect_startup_with(name, transport, startup, DEFAULT_TIMEOUT).await
    }

    /// Connect with one budget for the handshake and another for each request.
    ///
    /// A configured per-server `timeout` passes the same value for both, so it
    /// bounds the startup handshake and every later request. Without one the
    /// caller keeps the global startup budget and [`DEFAULT_TIMEOUT`].
    ///
    /// # Errors
    ///
    /// Returns [`McpError::Timeout`] naming `initialize` when `startup`
    /// elapses, and every error [`Server::connect_with`] can return.
    pub async fn connect_startup_with(
        name: &str,
        transport: Transport,
        startup: Duration,
        request: Duration,
    ) -> Result<Self, McpError> {
        match tokio::time::timeout(startup, Self::connect_with(name, transport, request)).await {
            Ok(result) => result,
            Err(_) => Err(McpError::Timeout {
                server: name.to_owned(),
                method: "initialize".to_owned(),
                timeout: startup,
            }),
        }
    }

    /// The configured server name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The protocol revision the server answered with.
    #[must_use]
    pub fn protocol_version(&self) -> &str {
        &self.protocol_version
    }

    /// The tools discovered during the handshake.
    #[must_use]
    pub fn tools(&self) -> &[McpTool] {
        &self.tools
    }

    /// Ask the server for its current tool list.
    ///
    /// # Errors
    ///
    /// Returns [`McpError`] when the request fails or the answer does not
    /// decode.
    pub async fn list_tools(&self) -> Result<Vec<McpTool>, McpError> {
        let result = self
            .connection
            .lock()
            .await
            .request("tools/list", json!({}))
            .await?;
        parse_tools(&self.name, &result)
    }

    /// Call one tool by its server-side name.
    ///
    /// An `isError` result is returned as [`CallResult::is_error`], not as an
    /// [`McpError`]: the server answered, and its text belongs to the model.
    ///
    /// # Errors
    ///
    /// Returns [`McpError`] when the call cannot be delivered or decoded.
    pub async fn call_tool(&self, tool: &str, arguments: Value) -> Result<CallResult, McpError> {
        let result = self
            .connection
            .lock()
            .await
            .request("tools/call", json!({"name": tool, "arguments": arguments}))
            .await?;
        CallResult::parse(&self.name, &result)
    }

    /// Adapt every discovered tool to the registry's tool trait.
    #[must_use]
    pub fn bridges(self: &Arc<Self>) -> Vec<ToolBridge> {
        self.tools
            .iter()
            .cloned()
            .map(|tool| ToolBridge::new(Arc::clone(self), tool))
            .collect()
    }

    /// Stop the stdio child, if this server has one. HTTP servers have nothing
    /// to close.
    pub async fn close(&self) {
        self.connection.lock().await.close();
    }
}

struct Connection {
    server: String,
    timeout: Duration,
    next_id: u64,
    wire: Wire,
    /// Why the wire can no longer be used, if it cannot. A stdio stream that
    /// lost a response cannot be resynchronised, so later calls fail loudly
    /// instead of reading the late answer as a reply to something else.
    dead: Option<String>,
}

impl Connection {
    async fn open(server: &str, transport: Transport, timeout: Duration) -> Result<Self, McpError> {
        let wire = match transport {
            Transport::Stdio {
                command,
                args,
                env,
                cwd,
            } => Wire::Stdio(Box::new(Stdio::spawn(
                server,
                &command,
                &args,
                &env,
                cwd.as_deref(),
            )?)),
            Transport::Http { url, headers } => Wire::Http(Http {
                client: http_client(server)?,
                url,
                headers,
                session_id: None,
            }),
        };
        Ok(Self {
            server: server.to_owned(),
            timeout,
            next_id: 1,
            wire,
            dead: None,
        })
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value, McpError> {
        if let Some(reason) = &self.dead {
            return Err(McpError::Closed {
                server: self.server.clone(),
                method: method.to_owned(),
                detail: reason.clone(),
            });
        }
        let id = self.next_id;
        self.next_id += 1;
        let message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let result = match &mut self.wire {
            Wire::Stdio(stdio) => {
                stdio
                    .request(&self.server, method, id, &message, self.timeout)
                    .await
            }
            Wire::Http(http) => {
                http.request(&self.server, method, id, &message, self.timeout)
                    .await
            }
        };
        if let Err(error) = &result {
            let fatal = match error {
                McpError::Io { .. }
                | McpError::Timeout { .. }
                | McpError::Exited { .. }
                | McpError::Malformed { .. }
                | McpError::Closed { .. } => true,
                McpError::Rpc { .. } | McpError::Http { .. } | McpError::Setup { .. } => false,
            };
            if fatal && matches!(&self.wire, Wire::Stdio(_)) {
                self.dead = Some(error.to_string());
            }
        }
        result
    }

    async fn notify(&mut self, method: &str, params: Value) -> Result<(), McpError> {
        if let Some(reason) = &self.dead {
            return Err(McpError::Closed {
                server: self.server.clone(),
                method: method.to_owned(),
                detail: reason.clone(),
            });
        }
        let message = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        match &mut self.wire {
            Wire::Stdio(stdio) => {
                stdio
                    .write(&self.server, method, &message, self.timeout)
                    .await
            }
            Wire::Http(http) => {
                http.notify(&self.server, method, &message, self.timeout)
                    .await
            }
        }
    }

    fn close(&mut self) {
        if let Wire::Stdio(stdio) = &mut self.wire {
            stdio.close();
        }
    }
}

enum Wire {
    Stdio(Box<Stdio>),
    Http(Http),
}

struct Stdio {
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
}

impl Stdio {
    fn spawn(
        server: &str,
        command: &str,
        args: &[String],
        env: &BTreeMap<String, String>,
        cwd: Option<&std::path::Path>,
    ) -> Result<Self, McpError> {
        let mut process = tokio::process::Command::new(command);
        process
            .args(args)
            .envs(env)
            .stdin(ProcessStdio::piped())
            .stdout(ProcessStdio::piped())
            // The TUI owns the terminal; a chatty server must not corrupt it.
            .stderr(ProcessStdio::null())
            .kill_on_drop(true);
        if let Some(cwd) = cwd {
            process.current_dir(cwd);
        }
        let mut child = process.spawn().map_err(|source| McpError::Io {
            server: server.to_owned(),
            method: "connect".to_owned(),
            source,
        })?;
        let stdin = child.stdin.take().ok_or_else(|| missing_pipe(server))?;
        let stdout = child.stdout.take().ok_or_else(|| missing_pipe(server))?;
        Ok(Self {
            child,
            stdin,
            lines: BufReader::new(stdout).lines(),
        })
    }

    async fn write(
        &mut self,
        server: &str,
        method: &str,
        message: &Value,
        timeout: Duration,
    ) -> Result<(), McpError> {
        let mut encoded = serde_json::to_vec(message).map_err(|error| McpError::Malformed {
            server: server.to_owned(),
            method: method.to_owned(),
            detail: error.to_string(),
        })?;
        encoded.push(b'\n');
        let written = tokio::time::timeout(timeout, async {
            self.stdin.write_all(&encoded).await?;
            self.stdin.flush().await
        })
        .await;
        match written {
            Err(_) => Err(McpError::Timeout {
                server: server.to_owned(),
                method: method.to_owned(),
                timeout,
            }),
            Ok(Err(source)) => Err(McpError::Io {
                server: server.to_owned(),
                method: method.to_owned(),
                source,
            }),
            Ok(Ok(())) => Ok(()),
        }
    }

    async fn request(
        &mut self,
        server: &str,
        method: &str,
        id: u64,
        message: &Value,
        timeout: Duration,
    ) -> Result<Value, McpError> {
        self.write(server, method, message, timeout).await?;
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(timeout_error(server, method, timeout));
            }
            let line = match tokio::time::timeout(remaining, self.lines.next_line()).await {
                Err(_) => return Err(timeout_error(server, method, timeout)),
                Ok(Err(source)) => {
                    return Err(McpError::Io {
                        server: server.to_owned(),
                        method: method.to_owned(),
                        source,
                    });
                }
                Ok(Ok(None)) => return Err(self.exited(server, method).await),
                Ok(Ok(Some(line))) => line,
            };
            if line.trim().is_empty() {
                continue;
            }
            let value: Value =
                serde_json::from_str(&line).map_err(|error| McpError::Malformed {
                    server: server.to_owned(),
                    method: method.to_owned(),
                    detail: error.to_string(),
                })?;
            if let Some(response) = response_for(&value, id) {
                return response_result(server, method, &response);
            }
            if let Some(reply) = server_request_reply(&value) {
                self.write(server, method, &reply, remaining).await?;
            }
        }
    }

    async fn exited(&mut self, server: &str, method: &str) -> McpError {
        let status = match self.child.try_wait() {
            Ok(Some(status)) => Some(status),
            _ => match tokio::time::timeout(Duration::from_millis(250), self.child.wait()).await {
                Ok(Ok(status)) => Some(status),
                _ => None,
            },
        };
        McpError::Exited {
            server: server.to_owned(),
            method: method.to_owned(),
            status: status.map_or_else(
                || "status unavailable".to_owned(),
                |status| status.to_string(),
            ),
        }
    }

    fn close(&mut self) {
        let _ = self.child.start_kill();
    }
}

struct Http {
    client: reqwest::Client,
    url: String,
    headers: BTreeMap<String, String>,
    session_id: Option<String>,
}

impl Http {
    async fn request(
        &mut self,
        server: &str,
        method: &str,
        id: u64,
        message: &Value,
        timeout: Duration,
    ) -> Result<Value, McpError> {
        let (status, content_type, body) = self.post(server, method, message, timeout).await?;
        if !(200..300).contains(&status) {
            return Err(McpError::Http {
                server: server.to_owned(),
                method: method.to_owned(),
                status,
            });
        }
        if body.trim().is_empty() {
            return Err(McpError::Malformed {
                server: server.to_owned(),
                method: method.to_owned(),
                detail: "the server sent no response body; a server that delivers responses on a \
                         separate event stream is not supported by this client"
                    .to_owned(),
            });
        }
        let values = if content_type.contains("text/event-stream") {
            sse_messages(&body)
        } else {
            vec![
                serde_json::from_str::<Value>(&body).map_err(|error| McpError::Malformed {
                    server: server.to_owned(),
                    method: method.to_owned(),
                    detail: error.to_string(),
                })?,
            ]
        };
        let response = values
            .iter()
            .find_map(|value| response_for(value, id))
            .ok_or_else(|| McpError::Malformed {
                server: server.to_owned(),
                method: method.to_owned(),
                detail: format!("no JSON-RPC response with id {id}"),
            })?;
        response_result(server, method, &response)
    }

    async fn notify(
        &mut self,
        server: &str,
        method: &str,
        message: &Value,
        timeout: Duration,
    ) -> Result<(), McpError> {
        let (status, _content_type, _body) = self.post(server, method, message, timeout).await?;
        if (200..300).contains(&status) {
            Ok(())
        } else {
            Err(McpError::Http {
                server: server.to_owned(),
                method: method.to_owned(),
                status,
            })
        }
    }

    /// POST one message and read the whole answer.
    ///
    /// The read is inside the timeout too: a streaming-only endpoint that never
    /// closes its body fails as [`McpError::Timeout`] rather than hanging.
    async fn post(
        &mut self,
        server: &str,
        method: &str,
        message: &Value,
        timeout: Duration,
    ) -> Result<(u16, String, String), McpError> {
        let mut request = self
            .client
            .post(&self.url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(
                reqwest::header::ACCEPT,
                "application/json, text/event-stream",
            )
            .json(message);
        for (name, value) in &self.headers {
            request = request.header(name, value);
        }
        if let Some(session) = &self.session_id {
            request = request.header("mcp-session-id", session);
        }

        let answered = tokio::time::timeout(timeout, async move {
            let response = request.send().await.map_err(|error| error.to_string())?;
            let status = response.status().as_u16();
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_owned();
            let session = response
                .headers()
                .get("mcp-session-id")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let body = response.text().await.map_err(|error| error.to_string())?;
            Ok::<_, String>((status, content_type, body, session))
        })
        .await;

        match answered {
            Err(_) => Err(timeout_error(server, method, timeout)),
            Ok(Err(detail)) => Err(McpError::Io {
                server: server.to_owned(),
                method: method.to_owned(),
                source: io::Error::other(detail),
            }),
            Ok(Ok((status, content_type, body, session))) => {
                if self.session_id.is_none() {
                    self.session_id = session;
                }
                Ok((status, content_type, body))
            }
        }
    }
}

fn http_client(server: &str) -> Result<reqwest::Client, McpError> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| McpError::Setup {
            server: server.to_owned(),
            method: "connect".to_owned(),
            detail: error.to_string(),
        })
}

/// Extract the response to `id` from one message or a batch.
///
/// A server *request* also carries an id; it is told apart by its `method`, so
/// an id collision cannot make a request look like a response.
fn response_for(value: &Value, id: u64) -> Option<Value> {
    match value {
        Value::Array(items) => items.iter().find_map(|item| response_for(item, id)),
        Value::Object(map) => {
            if map.contains_key("method") {
                return None;
            }
            if map.get("id").and_then(Value::as_u64) == Some(id) {
                Some(value.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Answer a server-initiated request, or `None` for a notification.
fn server_request_reply(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    let method = object.get("method")?.as_str()?;
    let id = object.get("id")?;
    if id.is_null() {
        return None;
    }
    Some(if method == "ping" {
        json!({"jsonrpc": "2.0", "id": id, "result": {}})
    } else {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32601,
                "message": format!("method not supported: {method}"),
            },
        })
    })
}

fn response_result(server: &str, method: &str, response: &Value) -> Result<Value, McpError> {
    if let Some(error) = response.get("error") {
        return Err(McpError::Rpc {
            server: server.to_owned(),
            method: method.to_owned(),
            code: error.get("code").and_then(Value::as_i64).unwrap_or(0),
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("(no message)")
                .to_owned(),
        });
    }
    response
        .get("result")
        .cloned()
        .ok_or_else(|| McpError::Malformed {
            server: server.to_owned(),
            method: method.to_owned(),
            detail: "the response has neither `result` nor `error`".to_owned(),
        })
}

fn timeout_error(server: &str, method: &str, timeout: Duration) -> McpError {
    McpError::Timeout {
        server: server.to_owned(),
        method: method.to_owned(),
        timeout,
    }
}

fn missing_pipe(server: &str) -> McpError {
    McpError::Io {
        server: server.to_owned(),
        method: "connect".to_owned(),
        source: io::Error::other("the child's stdin/stdout pipe is unavailable"),
    }
}

/// The JSON payloads in an SSE body.
///
/// Event names, ids, retries and comments are ignored; only `data:` lines
/// matter for a JSON-RPC response. A payload that is not JSON is dropped rather
/// than failing the whole body, so one stray event cannot hide the answer.
fn sse_messages(body: &str) -> Vec<Value> {
    let mut messages = Vec::new();
    let mut data = String::new();
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
        } else if line.trim().is_empty() {
            flush_sse(&mut data, &mut messages);
        }
    }
    flush_sse(&mut data, &mut messages);
    messages
}

fn flush_sse(data: &mut String, messages: &mut Vec<Value>) {
    if data.is_empty() {
        return;
    }
    if let Ok(value) = serde_json::from_str(data) {
        messages.push(value);
    }
    data.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_sse_body_yields_each_data_payload() {
        let body = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n\n\
                    : keep-alive\n\n\
                    data: {\"jsonrpc\":\"2.0\",\n\
                    data:  \"id\":2,\"result\":{\"ok\":true}}\n\n";
        let messages = sse_messages(body);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["id"], 1);
        assert_eq!(messages[1]["id"], 2);
        assert_eq!(messages[1]["result"]["ok"], true);
    }

    #[test]
    fn a_batch_response_is_searched_for_the_request_id() {
        let batch = json!([
            {"jsonrpc": "2.0", "id": 7, "result": {"answer": 42}},
            {"jsonrpc": "2.0", "id": 8, "result": {"answer": 0}}
        ]);
        let found = response_for(&batch, 7).expect("response");
        assert_eq!(found["result"]["answer"], 42);
        assert!(response_for(&batch, 9).is_none());
    }

    #[test]
    fn a_server_request_is_not_mistaken_for_a_response() {
        let request = json!({"jsonrpc": "2.0", "id": 7, "method": "roots/list"});
        assert!(response_for(&request, 7).is_none());
        let reply = server_request_reply(&request).expect("reply");
        assert_eq!(reply["error"]["code"], -32601);

        let ping = json!({"jsonrpc": "2.0", "id": 7, "method": "ping"});
        assert_eq!(
            server_request_reply(&ping).expect("reply")["result"],
            json!({})
        );

        let notification = json!({"jsonrpc": "2.0", "method": "notifications/progress"});
        assert!(server_request_reply(&notification).is_none());
    }

    #[test]
    fn a_json_rpc_error_is_reported_with_its_code_and_message() {
        let response =
            json!({"jsonrpc": "2.0", "id": 1, "error": {"code": -32601, "message": "nope"}});
        let error = response_result("fixture", "tools/call", &response).expect_err("error");
        assert!(
            matches!(error, McpError::Rpc { code: -32601, .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("fixture"));
        assert!(error.to_string().contains("tools/call"));
    }

    #[test]
    fn a_response_without_a_result_or_an_error_is_malformed() {
        let response = json!({"jsonrpc": "2.0", "id": 1});
        let error = response_result("fixture", "tools/list", &response).expect_err("error");
        assert!(matches!(error, McpError::Malformed { .. }), "{error:?}");
    }
}
