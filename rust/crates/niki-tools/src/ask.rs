//! The channel a tool uses to put a question to the person at the keyboard.
//!
//! [`Asker`] is deliberately small: one blocking call that returns the chosen
//! label or the text the person typed. The turn loop installs an implementation
//! through [`ToolContext::with_asker`](crate::ToolContext::with_asker); a
//! headless run installs none and the question tool reports that instead of
//! waiting.

use std::collections::VecDeque;
use std::sync::Mutex;

use thiserror::Error;

/// One choice offered with a question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskOption {
    /// The text returned to the model when this option is chosen.
    pub label: String,
    /// One line explaining the choice, for the screen that renders it.
    pub description: Option<String>,
}

/// A question put to the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskRequest {
    /// What the person is being asked.
    pub question: String,
    /// Two to four choices.
    pub options: Vec<AskOption>,
    /// Whether the person may type an answer instead of choosing.
    pub allow_freeform: bool,
}

/// What the person answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskAnswer {
    /// One of the offered options, by its label.
    Choice(String),
    /// Text the person typed; only offered when the request allowed free form.
    Freeform(String),
}

impl AskAnswer {
    /// The text a tool result carries.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Choice(label) => label,
            Self::Freeform(text) => text,
        }
    }
}

/// Why no answer came back.
#[derive(Debug, Error)]
pub enum AskError {
    /// No interactive user is attached to this run.
    #[error("no interactive user is attached")]
    NoUser,
    /// The asker failed, or the person dismissed the question.
    #[error("{0}")]
    Failed(String),
}

/// Puts one question to the person and waits for the answer.
///
/// The call is synchronous: it blocks the running turn until the answer
/// arrives. An implementation backed by a screen should hand the request to
/// that screen's thread and wait on a channel. Bounding the wait is the
/// implementation's business, not the tool's.
pub trait Asker: Send + Sync {
    /// Show `request` and return the answer, or why there is none.
    fn ask(&self, request: AskRequest) -> Result<AskAnswer, AskError>;
}

/// An [`Asker`] that answers from a fixed script, for tests.
#[derive(Debug, Default)]
pub struct ScriptedAsker {
    answers: Mutex<VecDeque<Result<AskAnswer, AskError>>>,
    requests: Mutex<Vec<AskRequest>>,
}

impl ScriptedAsker {
    /// Answer calls with `answers` in order; a call past the end fails.
    #[must_use]
    pub fn new(answers: impl IntoIterator<Item = AskAnswer>) -> Self {
        Self {
            answers: Mutex::new(answers.into_iter().map(Ok).collect()),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Fail the next call with `error`.
    #[must_use]
    pub fn failing(error: AskError) -> Self {
        Self {
            answers: Mutex::new(VecDeque::from([Err(error)])),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// The questions [`Asker::ask`] received, in order.
    #[must_use]
    pub fn requests(&self) -> Vec<AskRequest> {
        self.requests.lock().expect("lock").clone()
    }
}

impl Asker for ScriptedAsker {
    fn ask(&self, request: AskRequest) -> Result<AskAnswer, AskError> {
        self.requests.lock().expect("lock").push(request);
        self.answers
            .lock()
            .expect("lock")
            .pop_front()
            .unwrap_or_else(|| {
                Err(AskError::Failed(
                    "the scripted asker has no answers left".to_owned(),
                ))
            })
    }
}
