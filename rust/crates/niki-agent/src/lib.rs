//! The agent turn loop: model → tool calls → results → model.
//!
//! One [`Agent`] owns a provider, the workspace tool registry, the approval
//! policy and the conversation. [`Agent::run`] drives turns until the model
//! stops asking for tools, until [`AgentConfig::max_steps`] is reached, or until
//! the [`CancelHandle`] is raised.
//!
//! Events are reported through a callback as they happen so a caller can render
//! them live. The transcript is kept in [`Agent::messages`] and never rewritten,
//! so a cancelled turn keeps what it already produced (RULES.md 6).
//!
//! A command the policy gates as [`Decision::Ask`] goes to the [`Approver`] the
//! host installed; without one — a headless run — the request is reported to the
//! model as a refusal, because there is nobody to ask.
//!
//! The loop has no opinion about transport: any [`Provider`] streams the events
//! it already normalises, and any [`Registry`] tool is dispatched through the
//! same gate. That keeps the crate testable without a network (RULES.md 8).

#![forbid(unsafe_code)]

mod approval;
mod cancel;
mod error;
mod event;
mod turn;

pub use approval::{Approval, ApprovalRequest, Approver};
pub use cancel::CancelHandle;
pub use error::AgentError;
pub use event::{AgentEvent, RunOutcome};
pub use niki_policy::{AllowList, Decision, Mode};
pub use niki_provider::{Message, Provider, Role, StopReason, Usage};
pub use turn::{Agent, AgentConfig, DEFAULT_SYSTEM_PROMPT};
