//! Workspace tools for the Niki agent.
//!
//! Every tool is rooted at one [`ToolContext`]: a path argument is joined to the
//! workspace root, resolved through its nearest existing ancestor, and refused
//! when the result leaves the root (RULES.md 7). The crate carries the tools and
//! their schemas; approval policy lives in `niki-policy`, not here.
//!
//! [`Registry`] holds one instance of each built-in tool, plus any tool a host
//! registers at runtime — an MCP server's, for example. Providers receive
//! [`Registry::specs`] as the tool array of a request and dispatch a model's
//! tool call through [`Registry::get`].

mod args;
mod ask;
mod context;
mod error;
mod output;
mod registry;
mod tool;
mod tools;
mod walk;

pub use ask::{AskAnswer, AskError, AskOption, AskRequest, Asker, ScriptedAsker};
pub use context::ToolContext;
pub use error::ToolError;
pub use output::{MAX_OUTPUT_BYTES, MAX_OUTPUT_LINES, ToolOutput};
pub use registry::{Registry, ToolHandle};
pub use tool::{Tool, ToolFuture};
pub use tools::{
    EditFile, Glob, Grep, ListDir, ReadFile, RequestUserInput, Shell, WebFetch, WebSearch,
    WriteFile,
};

use std::io;

/// Largest file `grep` and `edit_file` will load whole into memory.
///
/// Both read a file in one call. A workspace can hold a database, a release
/// tarball, or a core dump; reading it whole would cost the machine more than
/// the search or the edit is worth, so anything larger is skipped by `grep` and
/// refused by `edit_file` with the limit named.
pub(crate) const MAX_LOADED_FILE_BYTES: u64 = 32 * 1024 * 1024;

pub(crate) fn io_error(path: &std::path::Path, source: io::Error) -> ToolError {
    ToolError::Io {
        path: path.display().to_string(),
        source,
    }
}
