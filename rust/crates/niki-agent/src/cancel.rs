use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Stops a running turn at the next checkpoint.
///
/// A UI thread cancels the handle; the turn loop reads it between events and
/// before each tool call, keeps the transcript it already produced, and returns
/// with a cancelled stop reason (RULES.md 6).
#[derive(Debug, Clone, Default)]
pub struct CancelHandle {
    flag: Arc<AtomicBool>,
}

impl CancelHandle {
    /// A handle that is not cancelled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ask the turn to stop.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    /// Whether a cancel has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}
