//! Approvals, the model picker and the help overlay for the live terminal UI.
//!
//! A modal is state plus a key handler; drawing needs a `Frame`, so the same
//! state is checked by driving keys and asserting the outcome, with no terminal
//! involved (RULES.md 8). This is the crate that turns [`Decision::Ask`] into a
//! real question: the approval modal answers
//! [`Approval`](../niki_agent/enum.Approval.html) values the turn loop waits on.
//!
//! [`Decision::Ask`]: niki_policy::Decision::Ask

#![forbid(unsafe_code)]

mod approval;
mod help;
mod model;
mod overlay;
mod threads;

pub use approval::ApprovalModal;
pub use help::{HelpOverlay, describe};
pub use model::{ModelChoice, ModelPicker};
pub use threads::{NO_PROMPT, ThreadEntry, ThreadPicker};

use crossterm::event::KeyEvent;
use niki_agent::Approval;
use niki_tui_themes::{GlyphMode, Palette};
use ratatui::Frame;
use ratatui::layout::Rect;

/// One overlay above the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    /// A gated command waiting for the person's answer.
    Approval(ApprovalModal),
    /// The models the configuration offers.
    Models(ModelPicker),
    /// The saved sessions, filterable, to resume one.
    Threads(ThreadPicker),
    /// The keymap.
    Help(HelpOverlay),
}

/// What a key press did to the modal it reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The key was consumed and the modal stays open.
    Stay,
    /// The modal closed without doing anything.
    Close,
    /// The person answered an approval request.
    Answer(Approval),
    /// The person chose a model spec.
    Model(String),
    /// The person chose a session to resume.
    Session(String),
}

impl Modal {
    /// The heading the overlay draws.
    #[must_use]
    pub fn title(&self) -> &'static str {
        match self {
            Self::Approval(_) => "Approve a command",
            Self::Models(_) => "Model",
            Self::Threads(_) => "Sessions",
            Self::Help(_) => "Keys",
        }
    }

    /// Handle one key press.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        match self {
            Self::Approval(modal) => modal.handle_key(key),
            Self::Models(modal) => modal.handle_key(key),
            Self::Threads(modal) => modal.handle_key(key),
            Self::Help(modal) => modal.handle_key(key),
        }
    }

    /// Draw the overlay over `area`.
    pub fn draw(&self, frame: &mut Frame, area: Rect, palette: &Palette, glyphs: GlyphMode) {
        match self {
            Self::Approval(modal) => overlay::approval(frame, area, modal, palette, glyphs),
            Self::Models(modal) => overlay::models(frame, area, modal, palette, glyphs),
            Self::Threads(modal) => overlay::threads(frame, area, modal, palette, glyphs),
            Self::Help(modal) => overlay::help(frame, area, modal, palette, glyphs),
        }
    }
}
