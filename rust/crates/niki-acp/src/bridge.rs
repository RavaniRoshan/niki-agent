//! The turn loop's events, as `session/update` notifications and stored rows.
//!
//! Nothing here re-implements the loop: the agent runs as it does for the
//! headless and full-screen entry points, and this maps what it reports onto
//! the protocol's session updates. Rows are written with the same roles the
//! full-screen app uses, so either surface can replay the other's transcript.

use std::sync::{Arc, Mutex, PoisonError};

use niki_agent::AgentEvent;
use niki_session::{SessionId, SessionStore};
use serde_json::json;

use crate::protocol;
use crate::rpc::Connection;
use crate::session::SessionHandle;

/// Maps one running turn onto notifications and the session store.
pub(crate) struct Emitter {
    session_id: String,
    store_id: Option<SessionId>,
    store: Option<Arc<Mutex<SessionStore>>>,
    connection: Connection,
    control: Arc<crate::session::Control>,
    text: String,
    warned: bool,
}

impl Emitter {
    pub(crate) fn new(
        session: &SessionHandle,
        connection: Connection,
        store: Option<Arc<Mutex<SessionStore>>>,
    ) -> Self {
        Self {
            session_id: session.id.clone(),
            store_id: session.store_id.clone(),
            store,
            connection,
            control: Arc::clone(&session.control),
            text: String::new(),
            warned: false,
        }
    }

    pub(crate) fn on_event(&mut self, event: &AgentEvent) {
        match event {
            AgentEvent::Text(delta) => {
                self.text.push_str(delta);
                self.notify(protocol::message_chunk("agent_message_chunk", delta, None));
            }
            AgentEvent::ToolStart { name, arguments } => {
                self.flush_text();
                let call = self.control.begin_call(name, arguments);
                let args = protocol::arguments_value(arguments);
                let presentation = protocol::present(name, &args);
                self.notify(protocol::tool_call_start(
                    &call.id,
                    &presentation,
                    protocol::raw_input(&args),
                ));
                self.persist("tool_call", &format!("{name}\n{arguments}"));
                if protocol::TODO_TOOLS.contains(&name.as_str()) {
                    if let Some(update) = protocol::plan_update(&args) {
                        self.notify(update);
                    }
                }
            }
            AgentEvent::ToolEnd {
                name,
                output,
                is_error,
            } => {
                if let Some(call) = self.control.end_call(name) {
                    let status = if *is_error { "failed" } else { "completed" };
                    // An edit already showed its diff in the start update; the
                    // reference server leaves its completion content empty too.
                    let content = (name != "edit_file").then_some(output.as_str());
                    self.notify(protocol::tool_call_update(&call.id, status, content));
                }
                let role = if *is_error {
                    "tool_error"
                } else {
                    "tool_result"
                };
                self.persist(role, &format!("{name}\n{output}"));
            }
            AgentEvent::Cancelled | AgentEvent::StepLimit { .. } => self.flush_text(),
            AgentEvent::Usage(_) => {}
        }
    }

    /// Close the turn: keep the last assistant text, if any.
    pub(crate) fn finish(&mut self) {
        self.flush_text();
    }

    /// Record a turn that ended in an error.
    pub(crate) fn fail(&mut self, message: &str) {
        self.flush_text();
        self.persist("error", message);
    }

    fn notify(&self, update: serde_json::Value) {
        self.connection.notify(
            "session/update",
            json!({ "sessionId": self.session_id, "update": update }),
        );
    }

    fn flush_text(&mut self) {
        if self.text.trim().is_empty() {
            self.text.clear();
            return;
        }
        let text = std::mem::take(&mut self.text);
        self.persist("assistant", &text);
    }

    fn persist(&mut self, role: &str, content: &str) {
        let (Some(store), Some(session)) = (&self.store, &self.store_id) else {
            return;
        };
        let result = store
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .append(session, role, content);
        if let Err(error) = result {
            if !self.warned {
                self.warned = true;
                // stderr, never stdout: the protocol owns stdout.
                eprintln!("niki-rs: warning: the session was not saved: {error}");
            }
        }
    }
}
