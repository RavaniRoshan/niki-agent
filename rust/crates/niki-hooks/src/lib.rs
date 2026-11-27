//! Lifecycle hooks for the Niki agent.
//!
//! A `hooks.json` document maps an event name to matcher groups, and each
//! group to the shell commands that run for it — the same shape the Python
//! client reads:
//!
//! ```json
//! {
//!   "hooks": {
//!     "PreToolUse": [
//!       {
//!         "matcher": "Bash",
//!         "hooks": [{ "type": "command", "command": "check-shell.sh" }]
//!       }
//!     ]
//!   }
//! }
//! ```
//!
//! Two files are read: `{workspace}/.niki/hooks.json` and the user file
//! `{config_dir}/hooks.json` (normally `~/.niki/hooks.json`). The project file
//! wins for an event it defines; the user file supplies the events it does not.
//! A missing file is normal; an unreadable or malformed one is a
//! [`Loaded::warnings`] entry, never a failed start.
//!
//! # The contract
//!
//! Supported events are [`Event::PreToolUse`], [`Event::PostToolUse`] and
//! [`Event::UserPromptSubmit`]; any other event name is ignored with a warning.
//! Tool matchers use the Python client's wire names (`shell` is `Bash`,
//! `write_file` is `Write`, MCP tools keep `mcp__<server>__<tool>`), so an
//! existing `hooks.json` keeps selecting the same tool.
//!
//! Each handler receives one JSON object on stdin with `hook_event_name`,
//! `cwd`, `session_id` when the host has one, and the event fields: `tool_name`,
//! `tool_input` and `tool_use_id` for the tool events, `tool_response` and
//! `duration_ms` for `PostToolUse`, and `prompt` for `UserPromptSubmit`.
//!
//! A handler influences the turn through its exit code and stdout:
//!
//! - exit `2` blocks the event; stderr is the reason;
//! - JSON `{"decision":"block","reason":…}` blocks it;
//! - on `PreToolUse`, JSON
//!   `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":…}}`
//!   also blocks it;
//! - `additionalContext` adds text to the tool result (or beside the prompt);
//!   `systemMessage` is shown to the person;
//! - `"continue": false` stops the event with `stopReason` as the reason.
//!
//! A blocked `PreToolUse` becomes the tool result
//! `"<tool> blocked by hook: <reason>"`, exactly the text the Python client
//! returns to the model. A blocked `UserPromptSubmit` stops the prompt before
//! the model sees it. On `PostToolUse` a block is feedback appended to the tool
//! result, not a refusal — the tool has already run.
//!
//! Every handler matched by an event runs, in configuration order, with its own
//! timeout (`600s` by default, `30s` for `UserPromptSubmit`). A command that
//! cannot start, exits non-zero for another reason, times out, or writes output
//! this build does not understand is a warning; the turn continues.
//! `permissionDecision` values `allow`, `ask` and `defer` are not implemented
//! in this build and are reported as warnings rather than silently ignored.
//!
//! Hooks run on the caller's thread and block it until the command finishes, so
//! the [`HookRunner`] seam is synchronous.
//!
//! A `[notify]` program in `config.toml` is separate from `hooks.json`:
//! [`Notify`] runs once at the end of a turn — completed, failed or cancelled —
//! with a JSON document on stdin. It is spawned detached and never blocks the
//! turn; a failure is one warning line on stderr.

#![forbid(unsafe_code)]

mod config;
mod matcher;
mod notify;
mod payload;
mod runner;

use std::path::Path;

use serde_json::Value;

pub use config::{Event, Hooks, Loaded};
pub use notify::{ANSWER_LIMIT, Notify, TurnEnd};
pub use payload::{PostToolUse, PreToolUse, ToolCall, UserPrompt, wire_tool_name};
pub use runner::Outcome;

/// The seam the agent loop calls.
///
/// [`Hooks`] is the real implementation; a host with no `hooks.json` installs
/// nothing, which is the same as every method returning [`Outcome::default`].
pub trait HookRunner: Send + Sync {
    /// Run `UserPromptSubmit` before the prompt reaches the model.
    fn user_prompt_submit(&self, request: &UserPrompt<'_>) -> Outcome;
    /// Run `PreToolUse` before a tool call; a block stops the call.
    fn pre_tool_use(&self, request: &PreToolUse<'_>) -> Outcome;
    /// Run `PostToolUse` after a tool call; context is appended to its result.
    fn post_tool_use(&self, request: &PostToolUse<'_>) -> Outcome;
}

impl HookRunner for Hooks {
    fn user_prompt_submit(&self, request: &UserPrompt<'_>) -> Outcome {
        let payload = payload::user_prompt_payload(request);
        self.dispatch(Event::UserPromptSubmit, None, &payload, request.cwd)
    }

    fn pre_tool_use(&self, request: &PreToolUse<'_>) -> Outcome {
        let payload = payload::pre_tool_use_payload(request);
        self.dispatch(
            Event::PreToolUse,
            Some(wire_tool_name(request.tool.name)),
            &payload,
            request.cwd,
        )
    }

    fn post_tool_use(&self, request: &PostToolUse<'_>) -> Outcome {
        let payload = payload::post_tool_use_payload(request);
        self.dispatch(
            Event::PostToolUse,
            Some(wire_tool_name(request.tool.name)),
            &payload,
            request.cwd,
        )
    }
}

impl Hooks {
    fn dispatch(&self, event: Event, target: Option<&str>, payload: &Value, cwd: &Path) -> Outcome {
        let mut outcome = Outcome::default();
        let Some(groups) = self.groups.get(&event) else {
            return outcome;
        };
        let wire_name = target.unwrap_or("");
        for group in groups {
            if target.is_some_and(|target| !group.matches(target)) {
                continue;
            }
            for handler in &group.handlers {
                runner::run_handler(event, handler, payload, cwd, wire_name, &mut outcome);
            }
        }
        outcome
    }
}
