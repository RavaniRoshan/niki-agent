use std::future::Future;
use std::pin::Pin;

use serde_json::Value;

use crate::{ToolContext, ToolError, ToolOutput};

/// The boxed future a [`Registry`](crate::Registry) hands back for a tool call.
pub type ToolFuture<'a> = Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>>;

/// A workspace tool the model can call.
///
/// `run` is written as a return-position `impl Future` rather than `async fn`
/// so the `Send` bound is part of the contract; implementations may still be
/// written as `async fn run`.
pub trait Tool: Send + Sync {
    /// The name a provider sees in a tool call.
    fn name(&self) -> &str;

    /// One sentence telling the model when to use the tool.
    fn description(&self) -> &str;

    /// JSON Schema for the argument object.
    fn schema(&self) -> Value;

    /// Whether the approval policy must ask the person before this tool runs.
    ///
    /// The default is `false`, which every built-in declares today: `shell` is
    /// gated by its command instead. A tool that acts outside the workspace —
    /// an MCP server, which may reach the network — overrides this so the agent
    /// sends it through the same approval callback a gated command uses. A
    /// server that declared the tool read-only keeps the default.
    fn requires_approval(&self) -> bool {
        false
    }

    /// Execute one call. Arguments are the model's JSON object.
    fn run(
        &self,
        args: Value,
        ctx: &ToolContext,
    ) -> impl Future<Output = Result<ToolOutput, ToolError>> + Send;
}
