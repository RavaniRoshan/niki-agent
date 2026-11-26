//! Agent Client Protocol server over stdin/stdout.
//!
//! An editor speaks newline-delimited JSON-RPC 2.0 to this crate: `initialize`,
//! `session/new`, `session/load`, `session/prompt`, `session/cancel`,
//! `session/set_mode` and `session/set_config_option`, with `session/update`
//! notifications flowing back for message chunks, tool calls and plans.
//!
//! The server is a host around the existing turn loop, not a second agent: a
//! prompt runs [`Agent::run`](niki_agent::Agent::run) against the host's
//! provider, tools and policy, a cancel raises the turn's handle, and the
//! session store gives `session/load` a transcript to replay. Permission is the
//! existing policy: a gated command or tool becomes a `session/request_permission`
//! request, and the editor's answer decides what the loop does.
//!
//! The host supplies [`AgentFactory`]; everything the protocol needs beyond
//! that — session ids, persistence, notifications, approvals — lives here.

#![forbid(unsafe_code)]

mod bridge;
mod protocol;
mod rpc;
mod serve;
mod server;
mod session;

pub use serve::{ServerError, serve, serve_stdio};
pub use server::{AgentFactory, ServerOptions, SessionSeed};
