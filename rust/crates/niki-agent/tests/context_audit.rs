//! Context audits: what one request injects, split into system prompt, tool
//! schemas and conversation.

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use niki_agent::{Agent, AgentConfig, audit_context};
use niki_provider::{ChatRequest, Message, Provider, ProviderError, StreamEvent};
use niki_tools::{Registry, Tool, ToolContext, ToolError, ToolOutput};
use serde_json::{Value, json};

/// A tool that exists only for its name, description and schema.
struct Stub {
    name: &'static str,
    description: &'static str,
    schema: Value,
}

impl Tool for Stub {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &str {
        self.description
    }

    fn schema(&self) -> Value {
        self.schema.clone()
    }

    async fn run(&self, _args: Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        Ok(ToolOutput::new("stub"))
    }
}

/// Two stubs of known text: `alpha` costs 84 characters, `beta` 64, each
/// divided by the same four-characters-per-token budget as the rest of the
/// crate.
fn registry_with_stubs() -> Registry {
    let mut registry = Registry::new();
    registry
        .register(Stub {
            name: "alpha",
            description: "Reads the alpha record",
            schema: json!({"type": "object", "properties": {"name": {"type": "string"}}}),
        })
        .register(Stub {
            name: "beta",
            description: "Reads beta records",
            schema: json!({"type": "array", "items": {"type": "number"}}),
        });
    registry
}

fn stub_tokens() -> usize {
    // 84 characters and 64 characters, rounded up by four.
    21 + 16
}

/// A provider that never streams: the audit reads agent state, not the wire.
struct Silent;

impl Provider for Silent {
    fn name(&self) -> &str {
        "silent"
    }

    fn stream(
        &self,
        _request: ChatRequest,
    ) -> BoxStream<'static, Result<StreamEvent, ProviderError>> {
        futures_util::stream::empty().boxed()
    }
}

#[test]
fn the_tool_array_costs_each_declared_schema_and_counts_every_tool() {
    let baseline = audit_context("", &[], &Registry::new());
    assert_eq!(baseline.tool_count, 7, "seven built-in tools");

    let audit = audit_context("", &[], &registry_with_stubs());

    assert_eq!(audit.tool_count, 9);
    assert_eq!(
        audit.tool_schema_tokens,
        baseline.tool_schema_tokens + stub_tokens(),
        "the two stubs add exactly their name, description and schema"
    );
}

#[test]
fn the_system_prompt_contributes_its_own_estimate() {
    let system = "y".repeat(40);
    let audit = audit_context(&system, &[], &Registry::new());

    assert_eq!(audit.system_tokens, 10);
    assert_eq!(audit.message_tokens, 0);
    assert_eq!(
        audit.total_tokens(),
        audit.tool_schema_tokens + 10,
        "the system prompt is part of the total"
    );

    let longer = audit_context(&"y".repeat(80), &[], &Registry::new());
    assert!(longer.system_tokens > audit.system_tokens);
}

#[test]
fn an_empty_history_costs_no_message_tokens() {
    assert_eq!(
        audit_context("system", &[], &Registry::new()).message_tokens,
        0
    );

    let history = vec![Message::user("abcd"), Message::assistant("efgh")];
    assert_eq!(
        audit_context("system", &history, &Registry::new()).message_tokens,
        2
    );
}

#[test]
fn the_total_is_the_sum_of_its_parts() {
    let history = vec![Message::user("find the tests")];
    let audit = audit_context("You are Niki.", &history, &registry_with_stubs());

    assert!(audit.system_tokens > 0);
    assert!(audit.tool_schema_tokens > 0);
    assert!(audit.message_tokens > 0);
    assert_eq!(
        audit.total_tokens(),
        audit.system_tokens + audit.tool_schema_tokens + audit.message_tokens
    );
}

#[test]
fn the_agent_audits_the_registry_it_dispatches_through() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = AgentConfig {
        model: "test-model".to_owned(),
        system: "abcdefgh".to_owned(),
        ..AgentConfig::default()
    };
    let registry = registry_with_stubs();
    let expected = audit_context(&config.system, &[], &registry);
    let agent = Agent::new(
        Box::new(Silent),
        ToolContext::new(dir.path()).expect("workspace"),
        config,
    )
    .with_registry(registry);

    assert_eq!(agent.context_audit(), expected);
    assert_eq!(agent.context_audit().tool_count, 9);
    assert_eq!(agent.context_audit().system_tokens, 2);
    assert_eq!(agent.context_audit().message_tokens, 0);
}
