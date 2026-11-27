use std::path::PathBuf;

use niki_policy::Mode;

use futures_util::future::BoxFuture;

/// What a gated action would do, with the details the dialog shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalAction {
    /// A shell command line, exactly as the model wrote it.
    Command(String),
    /// A write or edit to one file. The path is absolute; `inside` says whether
    /// it resolves inside the workspace root.
    Write {
        /// The tool that would write.
        tool: String,
        /// The absolute target path.
        path: PathBuf,
        /// Whether the target resolves inside the workspace.
        inside: bool,
    },
    /// Any other gated tool.
    Tool {
        /// The tool name.
        name: String,
        /// A one-line summary of its arguments.
        summary: String,
    },
}

/// A command the policy will not run without the person's consent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    /// What would run or change.
    pub action: ApprovalAction,
    /// A one-line label: the command, the absolute path, or the tool and its
    /// arguments. This is what a host without a dialog can show.
    pub command: String,
    /// The approval mode that asked, so the prompt can say why.
    pub mode: Mode,
}

impl ApprovalRequest {
    /// A shell command waiting for an answer.
    #[must_use]
    pub fn command(command: impl Into<String>, mode: Mode) -> Self {
        let command = command.into();
        Self {
            action: ApprovalAction::Command(command.clone()),
            command,
            mode,
        }
    }

    /// A file write waiting for an answer.
    ///
    /// `path` is the absolute target the dialog names, and `inside` is whether
    /// it resolves inside the workspace root.
    #[must_use]
    pub fn write(tool: impl Into<String>, path: PathBuf, inside: bool, mode: Mode) -> Self {
        Self {
            command: path.display().to_string(),
            action: ApprovalAction::Write {
                tool: tool.into(),
                path,
                inside,
            },
            mode,
        }
    }

    /// Any other gated tool waiting for an answer.
    #[must_use]
    pub fn tool(name: impl Into<String>, summary: impl Into<String>, mode: Mode) -> Self {
        let name = name.into();
        let summary = summary.into();
        let mut command = format!("{name} {summary}");
        if command.chars().count() > 160 {
            command = command.chars().take(157).collect::<String>() + "...";
        }
        Self {
            action: ApprovalAction::Tool { name, summary },
            command,
            mode,
        }
    }
}

/// What the person decided about one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approval {
    /// Run it this once.
    Once,
    /// Run it now, and every later action of the same class for the rest of this
    /// session without asking again.
    Always,
    /// Do not run it.
    Deny,
}

/// Answers one approval request.
///
/// The returned future resolves when the person decides, so a full-screen UI
/// can ask without blocking its own event loop. A run with no approver cannot
/// ask anybody and reports the request to the model as a refusal.
pub type Approver = Box<dyn Fn(ApprovalRequest) -> BoxFuture<'static, Approval> + Send + Sync>;
