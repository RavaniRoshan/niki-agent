//! One live protocol session: its workspace, mode, agent and pending calls.
//!
//! The turn loop reports a tool call's name and arguments on start and only the
//! name and output on end, so the session keeps a queue of started calls to
//! hand the matching id back. The call executing now is the one a permission
//! prompt describes, because the gate runs between start and end.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use niki_agent::{Agent, CancelHandle, Mode};
use niki_session::SessionId;

/// One tool call the model started, with the id the client saw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Call {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) arguments: String,
}

/// The state a cancel or a permission prompt may touch while a turn runs.
pub(crate) struct Control {
    pub(crate) cancel: CancelHandle,
    /// Whether a prompt is in flight for this session.
    pub(crate) running: AtomicBool,
    next_call: AtomicU64,
    live: Mutex<Live>,
}

#[derive(Default)]
struct Live {
    current: Option<Call>,
    queue: VecDeque<Call>,
}

impl Control {
    pub(crate) fn new(cancel: CancelHandle) -> Self {
        Self {
            cancel,
            running: AtomicBool::new(false),
            next_call: AtomicU64::new(0),
            live: Mutex::new(Live::default()),
        }
    }

    /// Record a call that is about to run and return the id the client sees.
    pub(crate) fn begin_call(&self, name: &str, arguments: &str) -> Call {
        let id = format!("call-{}", self.next_call.fetch_add(1, Ordering::SeqCst));
        let call = Call {
            id,
            name: name.to_owned(),
            arguments: arguments.to_owned(),
        };
        let mut live = self.lock_live();
        live.current = Some(call.clone());
        live.queue.push_back(call.clone());
        call
    }

    /// Match a finished call to its start, preferring the oldest of that name.
    pub(crate) fn end_call(&self, name: &str) -> Option<Call> {
        let mut live = self.lock_live();
        let index = live
            .queue
            .iter()
            .position(|call| call.name == name)
            .or_else(|| (!live.queue.is_empty()).then_some(0))?;
        let call = live.queue.remove(index)?;
        if live
            .current
            .as_ref()
            .is_some_and(|current| current.id == call.id)
        {
            live.current = None;
        }
        Some(call)
    }

    /// The call the gate is asking about right now, if there is one.
    pub(crate) fn current_call(&self) -> Option<Call> {
        self.lock_live().current.clone()
    }

    fn lock_live(&self) -> MutexGuard<'_, Live> {
        self.live.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A session the server holds, addressed by the id the client knows.
pub(crate) struct SessionHandle {
    pub(crate) id: String,
    pub(crate) cwd: PathBuf,
    pub(crate) store_id: Option<SessionId>,
    pub(crate) control: Arc<Control>,
    pub(crate) inner: tokio::sync::Mutex<Inner>,
}

/// The part of a session a running turn owns.
pub(crate) struct Inner {
    pub(crate) mode: Mode,
    pub(crate) agent: Option<Agent>,
}

impl SessionHandle {
    pub(crate) fn new(id: String, cwd: PathBuf, store_id: Option<SessionId>, mode: Mode) -> Self {
        Self {
            id,
            cwd,
            store_id,
            control: Arc::new(Control::new(CancelHandle::new())),
            inner: tokio::sync::Mutex::new(Inner { mode, agent: None }),
        }
    }

    pub(crate) async fn mode(&self) -> Mode {
        self.inner.lock().await.mode
    }
}

/// Whether two paths name the same directory, following symlinks when both
/// exist. A stored path written before canonicalisation still matches.
pub(crate) fn same_dir(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}
