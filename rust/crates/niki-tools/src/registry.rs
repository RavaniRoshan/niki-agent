use serde_json::{Value, json};

use crate::tool::ToolFuture;
use crate::tools::{EditFile, Glob, Grep, ListDir, ReadFile, Shell, WriteFile};
use crate::{Tool, ToolContext};

/// Object-safe view of a [`Tool`], used only to let the registry hold tools of
/// different types.
trait ErasedTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> Value;
    fn run<'a>(&'a self, args: Value, ctx: &'a ToolContext) -> ToolFuture<'a>;
}

impl<T: Tool> ErasedTool for T {
    fn name(&self) -> &str {
        Tool::name(self)
    }

    fn description(&self) -> &str {
        Tool::description(self)
    }

    fn schema(&self) -> Value {
        Tool::schema(self)
    }

    fn run<'a>(&'a self, args: Value, ctx: &'a ToolContext) -> ToolFuture<'a> {
        Box::pin(Tool::run(self, args, ctx))
    }
}

/// One tool call a caller obtained from [`Registry::get`].
#[derive(Clone, Copy)]
pub struct ToolHandle<'a> {
    tool: &'a dyn ErasedTool,
}

impl ToolHandle<'_> {
    /// See [`Tool::name`].
    pub fn name(&self) -> &str {
        self.tool.name()
    }

    /// See [`Tool::description`].
    pub fn description(&self) -> &str {
        self.tool.description()
    }

    /// See [`Tool::schema`].
    pub fn schema(&self) -> Value {
        self.tool.schema()
    }

    /// See [`Tool::run`].
    pub fn run<'a>(&'a self, args: Value, ctx: &'a ToolContext) -> ToolFuture<'a> {
        self.tool.run(args, ctx)
    }
}

/// Every workspace tool, keyed by name.
pub struct Registry {
    tools: Vec<Box<dyn ErasedTool>>,
}

impl Registry {
    /// Build the registry holding one instance of each tool.
    #[must_use]
    pub fn new() -> Self {
        let mut tools: Vec<Box<dyn ErasedTool>> = vec![
            Box::new(ReadFile),
            Box::new(WriteFile),
            Box::new(EditFile),
            Box::new(ListDir),
            Box::new(Shell::new()),
            Box::new(Grep),
            Box::new(Glob),
        ];
        tools.sort_by(|left, right| left.name().cmp(right.name()));
        Self { tools }
    }

    /// Look up a tool by its provider-visible name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<ToolHandle<'_>> {
        self.tools
            .iter()
            .find(|tool| tool.name() == name)
            .map(|tool| ToolHandle {
                tool: tool.as_ref(),
            })
    }

    /// The tool array a provider request carries: name, description and the
    /// JSON Schema of each tool's arguments.
    #[must_use]
    pub fn specs(&self) -> Value {
        Value::Array(
            self.tools
                .iter()
                .map(|tool| {
                    json!({
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": tool.schema(),
                    })
                })
                .collect(),
        )
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}
