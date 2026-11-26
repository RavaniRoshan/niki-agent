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

    /// Execute one call. Arguments are the model's JSON object.
    fn run(
        &self,
        args: Value,
        ctx: &ToolContext,
    ) -> impl Future<Output = Result<ToolOutput, ToolError>> + Send;
}
