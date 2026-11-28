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
mod connect;
mod help;
mod model;
mod overlay;
mod question;
mod settings;
mod status;
mod threads;
mod trust;

pub use approval::{ApprovalModal, ellipsize};
pub use connect::{CatalogEntry, ConnectModal, ConnectRequest, Stage};
pub use help::{HelpOverlay, describe};
pub use model::{ModelChoice, ModelPicker};
pub use question::{QuestionModal, QuestionReply, QuestionRow};
pub use settings::{Setting, SettingsModal, SettingsState};
pub use status::{StatusCard, StatusField};
pub use threads::{NO_PROMPT, ThreadEntry, ThreadPicker};
pub use trust::TrustModal;

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
    /// A structured question waiting for the person's answer.
    Question(QuestionModal),
    /// The models the configuration offers.
    Models(ModelPicker),
    /// The provider catalog and the key and model walk.
    Connect(ConnectModal),
    /// The saved sessions, filterable, to resume one.
    Threads(ThreadPicker),
    /// The `[ui]` settings.
    Settings(SettingsModal),
    /// The keymap.
    Help(HelpOverlay),
    /// The running session's facts, read-only.
    Status(StatusCard),
    /// The one-time workspace trust question.
    Trust(TrustModal),
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
    /// The person answered, or dismissed, a structured question.
    Question(QuestionReply),
    /// The person chose a model spec.
    Model(String),
    /// The person finished the provider walk.
    Connected(ConnectRequest),
    /// The person chose a session to resume.
    Session(String),
    /// The person changed one setting; the modal stays open.
    Setting(Setting),
    /// The person granted this workspace their trust.
    Trust,
}

impl Modal {
    /// The heading the overlay draws.
    #[must_use]
    pub fn title(&self) -> &'static str {
        match self {
            Self::Approval(_) => "Approve a command",
            Self::Question(_) => "Question",
            Self::Models(_) => "Model",
            Self::Connect(_) => "Connect a provider",
            Self::Threads(_) => "Sessions",
            Self::Settings(_) => "Settings",
            Self::Help(_) => "Keys",
            Self::Status(_) => "Status",
            Self::Trust(_) => "Trust this workspace",
        }
    }

    /// Handle one key press.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        match self {
            Self::Approval(modal) => modal.handle_key(key),
            Self::Question(modal) => modal.handle_key(key),
            Self::Models(modal) => modal.handle_key(key),
            Self::Connect(modal) => modal.handle_key(key),
            Self::Threads(modal) => modal.handle_key(key),
            Self::Settings(modal) => modal.handle_key(key),
            Self::Help(modal) => modal.handle_key(key),
            Self::Status(modal) => modal.handle_key(key),
            Self::Trust(modal) => modal.handle_key(key),
        }
    }

    /// Draw the overlay over `area`.
    pub fn draw(&self, frame: &mut Frame, area: Rect, palette: &Palette, glyphs: GlyphMode) {
        match self {
            Self::Approval(modal) => overlay::approval(frame, area, modal, palette, glyphs),
            Self::Question(modal) => overlay::question(frame, area, modal, palette, glyphs),
            Self::Models(modal) => overlay::models(frame, area, modal, palette, glyphs),
            Self::Connect(modal) => overlay::connect(frame, area, modal, palette, glyphs),
            Self::Threads(modal) => overlay::threads(frame, area, modal, palette, glyphs),
            Self::Settings(modal) => overlay::settings(frame, area, modal, palette, glyphs),
            Self::Help(modal) => overlay::help(frame, area, modal, palette, glyphs),
            Self::Status(modal) => overlay::status(frame, area, modal, palette, glyphs),
            Self::Trust(modal) => overlay::trust(frame, area, modal, palette, glyphs),
        }
    }
}
