//! Protocol state and the method table.
//!
//! Each method maps onto the existing turn loop and session store: a prompt
//! runs [`Agent::run`] and streams what it reports, a cancel raises the turn's
//! [`CancelHandle`], and a load replays rows the store already holds. The
//! approval policy is unchanged — a gated command or tool asks the editor over
//! `session/request_permission` and waits for the answer.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::future::BoxFuture;
use niki_agent::{
    Agent, Approval, ApprovalRequest, Approver, CancelHandle, Mode, RunOutcome, StopReason,
};
use niki_session::{Message, SessionStore};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::bridge::Emitter;
use crate::protocol::{self, ContentBlock};
use crate::rpc::{Connection, RpcError};
use crate::session::{Call, Control, SessionHandle, same_dir};

/// Everything one session's agent needs from the host.
pub struct SessionSeed {
    /// The id the client uses for this session.
    pub session_id: String,
    /// Canonical workspace root the session was opened on.
    pub cwd: PathBuf,
    /// Mode in force when the agent is built.
    pub mode: Mode,
    /// Raised when the client sends `session/cancel`.
    pub cancel: CancelHandle,
    /// Asks the editor for permission, and waits for its answer.
    pub approver: Approver,
}

/// Builds the agent for one session.
///
/// The host owns provider construction and configuration; the server owns the
/// protocol, the session store and the approval round-trip.
pub type AgentFactory = Box<dyn Fn(SessionSeed) -> Result<Agent, String> + Send + Sync>;

/// How the server behaves outside the protocol.
#[derive(Debug, Clone)]
pub struct ServerOptions {
    /// SQLite file to persist sessions in, or `None` to keep them in memory.
    pub store: Option<PathBuf>,
    /// Mode a new session starts in.
    pub default_mode: Mode,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            store: None,
            default_mode: Mode::Manual,
        }
    }
}

impl ServerOptions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Persist sessions at `path`, which `serve` opens.
    #[must_use]
    pub fn with_store(mut self, path: impl Into<PathBuf>) -> Self {
        self.store = Some(path.into());
        self
    }

    /// Start new sessions in `mode`.
    #[must_use]
    pub fn with_default_mode(mut self, mode: Mode) -> Self {
        self.default_mode = mode;
        self
    }
}

type Factory = dyn Fn(SessionSeed) -> Result<Agent, String> + Send + Sync;

pub(crate) struct Server {
    factory: Arc<Factory>,
    connection: Connection,
    store: Option<Arc<Mutex<SessionStore>>>,
    sessions: tokio::sync::Mutex<HashMap<String, Arc<SessionHandle>>>,
    default_mode: Mode,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeParams {
    protocol_version: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewSessionParams {
    cwd: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoadSessionParams {
    cwd: String,
    session_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptParams {
    session_id: String,
    prompt: Vec<ContentBlock>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CancelParams {
    session_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetModeParams {
    session_id: String,
    mode_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetConfigOptionParams {
    session_id: String,
    config_id: String,
    value: Value,
}

impl Server {
    pub(crate) fn new(
        factory: Arc<Factory>,
        connection: Connection,
        store: Option<Arc<Mutex<SessionStore>>>,
        default_mode: Mode,
    ) -> Self {
        Self {
            factory,
            connection,
            store,
            sessions: tokio::sync::Mutex::new(HashMap::new()),
            default_mode,
        }
    }

    pub(crate) fn respond(&self, id: Value, outcome: Result<Value, RpcError>) {
        self.connection.respond(id, outcome);
    }

    /// The methods this server implements.
    ///
    /// `authenticate`, `session/list` and the unstable methods are not in the
    /// table, so they answer `-32601` exactly as the reference server does when
    /// an agent does not implement them.
    pub(crate) async fn route(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        match method {
            "initialize" => self.initialize(params),
            "session/new" => self.new_session(params).await,
            "session/load" => self.load_session(params).await,
            "session/prompt" => self.prompt(params).await,
            "session/set_mode" => self.set_mode(params).await,
            "session/set_config_option" => self.set_config_option(params).await,
            other => Err(RpcError::method_not_found(other)),
        }
    }

    pub(crate) async fn notification(&self, method: &str, params: Value) {
        if method != "session/cancel" {
            return;
        }
        let Ok(params) = serde_json::from_value::<CancelParams>(params) else {
            return;
        };
        if let Some(session) = self.session(&params.session_id).await {
            session.control.cancel.cancel();
        }
    }

    fn initialize(&self, params: Value) -> Result<Value, RpcError> {
        let params = parse::<InitializeParams>(params)?;
        Ok(json!({
            "protocolVersion": params.protocol_version,
            "agentCapabilities": {
                "loadSession": self.store.is_some(),
                "promptCapabilities": { "image": false, "audio": false },
            },
            "agentInfo": {
                "name": "niki",
                "title": "Niki Agent",
                "version": env!("CARGO_PKG_VERSION"),
            },
        }))
    }

    async fn new_session(&self, params: Value) -> Result<Value, RpcError> {
        let params = parse::<NewSessionParams>(params)?;
        let cwd = canonical_cwd(&params.cwd)?;
        let (id, store_id) = match &self.store {
            Some(store) => {
                let created = store
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .create(&cwd)
                    .map_err(|error| RpcError::internal(format!("session store: {error}")))?;
                (created.as_str().to_owned(), Some(created))
            }
            None => (fresh_session_id(), None),
        };
        let handle = Arc::new(SessionHandle::new(
            id.clone(),
            cwd,
            store_id,
            self.default_mode,
        ));
        self.sessions.lock().await.insert(id.clone(), handle);
        Ok(json!({
            "sessionId": id,
            "modes": protocol::modes_state(self.default_mode),
            "configOptions": protocol::config_options(self.default_mode),
        }))
    }

    async fn load_session(&self, params: Value) -> Result<Value, RpcError> {
        let Some(store) = &self.store else {
            return Err(RpcError::method_not_found("session/load"));
        };
        let params = parse::<LoadSessionParams>(params)?;
        let cwd = canonical_cwd(&params.cwd)?;

        let summary = store
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .list()
            .map_err(|error| RpcError::internal(format!("session store: {error}")))?
            .into_iter()
            .find(|summary| summary.id.as_str() == params.session_id);
        let Some(summary) = summary else {
            return Err(RpcError::resource_not_found(&params.session_id));
        };
        if !same_dir(std::path::Path::new(&summary.cwd), &cwd) {
            return Err(RpcError::invalid_params_field(
                "cwd",
                "must match the working directory used to create the session",
            ));
        }

        let existing = self.session(&params.session_id).await;
        if existing
            .as_ref()
            .is_some_and(|session| session.control.running.load(Ordering::SeqCst))
        {
            return Err(RpcError::internal(
                "a prompt is running for this session; load cannot replay it",
            ));
        }
        if let Some(session) = &existing {
            if !same_dir(&session.cwd, &cwd) {
                return Err(RpcError::invalid_params_field(
                    "cwd",
                    "must match the working directory used to create the session",
                ));
            }
        }

        // Read the whole transcript before replaying it, so a session write
        // that starts mid-replay cannot produce a torn transcript.
        let rows = store
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .load(&summary.id)
            .map_err(|error| RpcError::internal(format!("session store: {error}")))?;

        let session = match existing {
            Some(session) => session,
            None => {
                let session = Arc::new(SessionHandle::new(
                    params.session_id.clone(),
                    cwd,
                    Some(summary.id),
                    self.default_mode,
                ));
                self.sessions
                    .lock()
                    .await
                    .insert(params.session_id.clone(), Arc::clone(&session));
                session
            }
        };
        self.replay(&session, &rows);
        let mode = session.mode().await;
        Ok(json!({
            "modes": protocol::modes_state(mode),
            "configOptions": protocol::config_options(mode),
        }))
    }

    async fn prompt(&self, params: Value) -> Result<Value, RpcError> {
        let params = parse::<PromptParams>(params)?;
        let text = protocol::prompt_text(&params.prompt)?;
        let session = self
            .session(&params.session_id)
            .await
            .ok_or_else(|| RpcError::resource_not_found(&params.session_id))?;
        if session.control.running.swap(true, Ordering::SeqCst) {
            return Err(RpcError::internal(
                "a prompt is already running for this session",
            ));
        }
        let outcome = self.run_prompt(&session, &text).await;
        session.control.running.store(false, Ordering::SeqCst);
        outcome
    }

    async fn run_prompt(&self, session: &SessionHandle, text: &str) -> Result<Value, RpcError> {
        let mut inner = session.inner.lock().await;
        if inner.agent.is_none() {
            let seed = SessionSeed {
                session_id: session.id.clone(),
                cwd: session.cwd.clone(),
                mode: inner.mode,
                cancel: session.control.cancel.clone(),
                approver: permission_approver(
                    self.connection.clone(),
                    Arc::clone(&session.control),
                    session.id.clone(),
                ),
            };
            let agent = (self.factory)(seed).map_err(RpcError::internal)?;
            inner.agent = Some(agent);
        }
        let Some(agent) = inner.agent.as_mut() else {
            return Err(RpcError::internal("the session agent was not built"));
        };

        session.control.cancel.reset();
        self.append(session, "user", text);
        let mut emitter = Emitter::new(session, self.connection.clone(), self.store.clone());
        let outcome = agent.run(text, &mut |event| emitter.on_event(event)).await;
        emitter.finish();
        match outcome {
            Ok(outcome) => Ok(json!({ "stopReason": stop_reason(&outcome) })),
            Err(error) => {
                let message = error.to_string();
                emitter.fail(&message);
                Err(RpcError::internal(message))
            }
        }
    }

    async fn set_mode(&self, params: Value) -> Result<Value, RpcError> {
        let params = parse::<SetModeParams>(params)?;
        let mode = protocol::mode_from_id(&params.mode_id).ok_or_else(|| {
            RpcError::invalid_params_message(format!("Invalid mode: {}", params.mode_id))
        })?;
        let session = self
            .session(&params.session_id)
            .await
            .ok_or_else(|| RpcError::resource_not_found(&params.session_id))?;
        let mut inner = session.inner.lock().await;
        if let Some(agent) = inner.agent.as_mut() {
            agent.set_mode(mode);
        }
        inner.mode = mode;
        Ok(json!({}))
    }

    async fn set_config_option(&self, params: Value) -> Result<Value, RpcError> {
        let params = parse::<SetConfigOptionParams>(params)?;
        if params.config_id != "mode" {
            return Err(RpcError::invalid_params_message(format!(
                "Unknown config option: {}",
                params.config_id
            )));
        }
        let Value::String(mode_id) = params.value else {
            return Err(RpcError::invalid_params_message(
                "Config option `mode` expects a string value",
            ));
        };
        let mode = protocol::mode_from_id(&mode_id)
            .ok_or_else(|| RpcError::invalid_params_message(format!("Invalid mode: {mode_id}")))?;
        let session = self
            .session(&params.session_id)
            .await
            .ok_or_else(|| RpcError::resource_not_found(&params.session_id))?;
        let mut inner = session.inner.lock().await;
        if let Some(agent) = inner.agent.as_mut() {
            agent.set_mode(mode);
        }
        inner.mode = mode;
        Ok(json!({ "configOptions": protocol::config_options(mode) }))
    }

    async fn session(&self, id: &str) -> Option<Arc<SessionHandle>> {
        self.sessions.lock().await.get(id).cloned()
    }

    fn append(&self, session: &SessionHandle, role: &str, content: &str) {
        let (Some(store), Some(id)) = (&self.store, &session.store_id) else {
            return;
        };
        if let Err(error) = store
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .append(id, role, content)
        {
            eprintln!("niki: warning: the session was not saved: {error}");
        }
    }

    /// Replay stored rows as the session updates a client renders.
    fn replay(&self, session: &SessionHandle, rows: &[Message]) {
        let mut pending: VecDeque<Call> = VecDeque::new();
        let mut replay_calls = 0usize;
        for row in rows {
            let message_id = row.id.to_string();
            match row.role.as_str() {
                "user" => self.notify(
                    session,
                    protocol::message_chunk("user_message_chunk", &row.content, Some(&message_id)),
                ),
                "assistant" => self.notify(
                    session,
                    protocol::message_chunk("agent_message_chunk", &row.content, Some(&message_id)),
                ),
                "tool_call" => {
                    let (name, arguments) =
                        row.content.split_once('\n').unwrap_or((&row.content, ""));
                    let id = format!("replay-{replay_calls}");
                    replay_calls += 1;
                    let args = protocol::arguments_value(arguments);
                    let presentation = protocol::present(name, &args);
                    self.notify(
                        session,
                        protocol::tool_call_start(&id, &presentation, protocol::raw_input(&args)),
                    );
                    if protocol::TODO_TOOLS.contains(&name) {
                        if let Some(update) = protocol::plan_update(&args) {
                            self.notify(session, update);
                        }
                    }
                    pending.push_back(Call {
                        id,
                        name: name.to_owned(),
                        arguments: arguments.to_owned(),
                    });
                }
                "tool_result" | "tool_error" => {
                    let (name, output) = row.content.split_once('\n').unwrap_or((&row.content, ""));
                    let index = pending.iter().position(|call| call.name == name);
                    if let Some(index) = index {
                        if let Some(call) = pending.remove(index) {
                            let status = if row.role == "tool_error" {
                                "failed"
                            } else {
                                "completed"
                            };
                            let content = (name != "edit_file").then_some(output);
                            self.notify(
                                session,
                                protocol::tool_call_update(&call.id, status, content),
                            );
                        }
                    }
                }
                // A provider failure row has no protocol update of its own; the
                // editor received the error on the prompt it answered.
                _ => {}
            }
        }
    }

    fn notify(&self, session: &SessionHandle, update: Value) {
        self.connection.notify(
            "session/update",
            json!({ "sessionId": session.id, "update": update }),
        );
    }
}

fn parse<T: for<'de> Deserialize<'de>>(params: Value) -> Result<T, RpcError> {
    serde_json::from_value(params).map_err(|error| RpcError::invalid_params(error.to_string()))
}

fn canonical_cwd(raw: &str) -> Result<PathBuf, RpcError> {
    let path = PathBuf::from(raw);
    let canonical = path
        .canonicalize()
        .map_err(|_| RpcError::invalid_params_field("cwd", "must be an existing directory"))?;
    if !canonical.is_dir() {
        return Err(RpcError::invalid_params_field(
            "cwd",
            "must be an existing directory",
        ));
    }
    Ok(canonical)
}

fn fresh_session_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!(
        "acp-{nanos:032x}-{:02x}",
        COUNTER.fetch_add(1, Ordering::SeqCst)
    )
}

fn stop_reason(outcome: &RunOutcome) -> &'static str {
    match &outcome.stop {
        StopReason::EndTurn | StopReason::ToolUse => "end_turn",
        StopReason::MaxTokens => "max_tokens",
        StopReason::Other(reason) if reason == "cancelled" => "cancelled",
        StopReason::Other(reason) if reason.starts_with("step limit") => "max_turn_requests",
        StopReason::Other(_) => "end_turn",
    }
}

/// Ask the editor about one gated call and translate its answer.
fn permission_approver(
    connection: Connection,
    control: Arc<Control>,
    session_id: String,
) -> Approver {
    Box::new(move |request: ApprovalRequest| {
        let connection = connection.clone();
        let control = Arc::clone(&control);
        let session_id = session_id.clone();
        let future = async move {
            let (tool_call_id, title, raw_input) = match control.current_call() {
                Some(call) => {
                    let args = protocol::arguments_value(&call.arguments);
                    let presentation = protocol::present(&call.name, &args);
                    (call.id, presentation.title, protocol::raw_input(&args))
                }
                None => ("permission".to_owned(), request.command.clone(), None),
            };
            let mut tool_call = json!({ "toolCallId": tool_call_id, "title": title });
            if let Some(raw_input) = raw_input {
                tool_call["rawInput"] = raw_input;
            }
            let params = json!({
                "sessionId": session_id,
                "toolCall": tool_call,
                "options": [
                    { "optionId": "approve", "name": "Approve", "kind": "allow_once" },
                    { "optionId": "reject", "name": "Reject", "kind": "reject_once" },
                    { "optionId": "approve_always", "name": "Always allow", "kind": "allow_always" },
                ],
            });
            match connection
                .request("session/request_permission", params)
                .await
            {
                Ok(response) => decision(&response),
                // A client that cannot be reached cannot approve anything.
                Err(_) => Approval::Deny,
            }
        };
        Box::pin(future) as BoxFuture<'static, Approval>
    })
}

fn decision(response: &Value) -> Approval {
    let outcome = &response["outcome"];
    if outcome["outcome"].as_str() != Some("selected") {
        return Approval::Deny;
    }
    match outcome["optionId"].as_str() {
        Some("approve") => Approval::Once,
        Some("approve_always") => Approval::Always,
        _ => Approval::Deny,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_permission_answer_maps_onto_the_policy_decision() {
        assert_eq!(
            decision(&json!({ "outcome": { "outcome": "selected", "optionId": "approve" } })),
            Approval::Once
        );
        assert_eq!(
            decision(&json!({
                "outcome": { "outcome": "selected", "optionId": "approve_always" },
            })),
            Approval::Always
        );
        assert_eq!(
            decision(&json!({ "outcome": { "outcome": "selected", "optionId": "reject" } })),
            Approval::Deny
        );
        assert_eq!(
            decision(&json!({ "outcome": { "outcome": "cancelled" } })),
            Approval::Deny
        );
        assert_eq!(decision(&json!({})), Approval::Deny);
    }

    #[test]
    fn a_finished_run_maps_onto_a_protocol_stop_reason() {
        let outcome = |stop: StopReason| RunOutcome {
            text: String::new(),
            steps: 1,
            tool_calls: 0,
            stop,
            usage: niki_agent::Usage::default(),
        };
        assert_eq!(stop_reason(&outcome(StopReason::EndTurn)), "end_turn");
        assert_eq!(stop_reason(&outcome(StopReason::MaxTokens)), "max_tokens");
        assert_eq!(
            stop_reason(&outcome(StopReason::Other("cancelled".to_owned()))),
            "cancelled"
        );
        assert_eq!(
            stop_reason(&outcome(StopReason::Other(
                "step limit 2 reached".to_owned()
            ))),
            "max_turn_requests"
        );
    }

    #[test]
    fn a_relative_or_missing_working_directory_is_refused() {
        let error = canonical_cwd("/definitely/not/a/directory/here").expect_err("refused");
        assert_eq!(error.to_value()["code"], json!(-32602));
    }
}
