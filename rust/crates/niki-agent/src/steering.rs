//! Text a screen hands to a turn that is already running.
//!
//! A screen cannot borrow the agent while [`Agent::run`](crate::Agent::run) has
//! it, so [`Steering`] is the one shared point between them: the screen pushes a
//! line, and the turn loop takes what arrived before it builds its next request.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Prompts steered into a running turn, oldest first.
#[derive(Debug, Clone, Default)]
pub struct Steering {
    pending: Arc<Mutex<Vec<String>>>,
}

impl Steering {
    /// An empty queue.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Hand one line to the turn that is running.
    ///
    /// A blank line is dropped: there is nothing for the model to read.
    pub fn push(&self, text: impl Into<String>) {
        let text = text.into();
        if text.trim().is_empty() {
            return;
        }
        self.lock().push(text);
    }

    /// Take everything queued since the last call.
    pub(crate) fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.lock())
    }

    fn lock(&self) -> MutexGuard<'_, Vec<String>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
