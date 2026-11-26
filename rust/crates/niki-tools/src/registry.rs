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
    fn requires_approval(&self) -> bool;
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

    fn requires_approval(&self) -> bool {
        Tool::requires_approval(self)
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

    /// See [`Tool::requires_approval`].
    pub fn requires_approval(&self) -> bool {
        self.tool.requires_approval()
    }

    /// See [`Tool::run`].
    pub fn run<'a>(&'a self, args: Value, ctx: &'a ToolContext) -> ToolFuture<'a> {
        self.tool.run(args, ctx)
    }
}

/// Every tool, keyed by name: the built-ins plus anything registered later.
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

    /// Add a tool discovered at runtime — an MCP server's, for example — so the
    /// agent loop dispatches it like a built-in.
    ///
    /// Registration keeps [`Registry::specs`] in name order and names unique: a
    /// tool registered with an existing name replaces it.
    pub fn register<T: Tool + 'static>(&mut self, tool: T) -> &mut Self {
        let name = tool.name().to_owned();
        self.tools.retain(|existing| existing.name() != name);
        self.tools.push(Box::new(tool));
        self.tools
            .sort_by(|left, right| left.name().cmp(right.name()));
        self
    }

    /// The registered tool names, in the order [`Registry::specs`] reports them.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.tools.iter().map(|tool| tool.name()).collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ToolError, ToolOutput};

    struct External {
        gated: bool,
    }

    impl Tool for External {
        fn name(&self) -> &str {
            "mcp__fixture__write"
        }

        fn description(&self) -> &str {
            "An external mutating tool."
        }

        fn schema(&self) -> Value {
            json!({"type": "object"})
        }

        fn requires_approval(&self) -> bool {
            self.gated
        }

        async fn run(&self, _args: Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::new("ran"))
        }
    }

    #[test]
    fn a_registered_tool_is_dispatched_and_keeps_its_approval_flag() {
        let mut registry = Registry::new();
        registry.register(External { gated: true });

        let handle = registry.get("mcp__fixture__write").expect("registered");
        assert!(handle.requires_approval());
        assert!(
            !registry
                .get("write_file")
                .expect("built-in")
                .requires_approval()
        );

        let Value::Array(specs) = registry.specs() else {
            panic!("specs are an array");
        };
        let names: Vec<&str> = specs
            .iter()
            .filter_map(|spec| spec.get("name").and_then(Value::as_str))
            .collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "registration keeps the specs sorted");
        assert!(names.contains(&"mcp__fixture__write"));
        assert_eq!(registry.names(), names, "names and specs agree");
    }
}
