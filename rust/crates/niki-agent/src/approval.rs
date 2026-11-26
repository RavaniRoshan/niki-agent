use niki_policy::Mode;

use futures_util::future::BoxFuture;

/// A command the policy will not run without the person's consent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    /// The command line as the model wrote it.
    pub command: String,
    /// The approval mode that asked, so the prompt can say why.
    pub mode: Mode,
}

/// What the person decided about one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approval {
    /// Run it this once.
    Once,
    /// Run it now, and every later identical command without asking again.
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
