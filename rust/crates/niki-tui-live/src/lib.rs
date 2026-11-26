//! The full-screen terminal application.
//!
//! Three regions, top to bottom: the transcript, the composer and the status
//! bar. The agent runs on a worker thread ([`WorkerHandle`]); its events arrive
//! on a channel, fold into [`App`], and the screen redraws. Cancelling stops new
//! work and keeps the partial transcript (RULES.md 6).
//!
//! The state layer has no terminal in it, so the whole view is checked by
//! feeding events and asserting the resulting state and rendered buffer.

#![forbid(unsafe_code)]

mod app;
mod composer;
mod run;
mod scroll;
mod status;
mod transcript;
mod ui;
mod worker;

pub use app::{App, Command, SessionInfo, WHEEL_LINES};
pub use composer::Composer;
pub use niki_tui_modals::{Modal, ModelPicker, Outcome};
pub use run::run;
pub use scroll::Scroll;
pub use status::{Status, TurnState};
pub use transcript::{Row, RowKind, Transcript};
pub use ui::{PLACEHOLDER, draw};
pub use worker::{SessionParts, WorkerEvent, WorkerHandle};
