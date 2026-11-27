//! A `task` tool that hands one self-contained job to a child agent.
//!
//! The child runs its own turn loop over a registry derived from the session's,
//! without [`TOOL_NAME`] itself, and the parent receives only the answer and a
//! one-line summary. The child shares the parent's [`CancelHandle`], starts
//! with an empty history, and stops after a bounded number of model turns. A
//! child that cannot be built or loses its provider is a [`ToolError`] the
//! model can read.

#![forbid(unsafe_code)]

use std::env;

use niki_agent::{Agent, CancelHandle, RunOutcome, StopReason};
use niki_tools::{Registry, Tool, ToolContext, ToolError, ToolOutput};
use serde_json::{Value, json};

/// The name the model calls and the name a child never sees.
pub const TOOL_NAME: &str = "task";

/// Model turns a child may take unless the environment overrides it.
pub const DEFAULT_STEP_BUDGET: u32 = 20;

/// Environment variable that overrides [`DEFAULT_STEP_BUDGET`].
pub const STEP_BUDGET_ENV: &str = "NIKI_SUBAGENT_MAX_STEPS";

type ToolSource = Box<dyn Fn() -> Registry + Send + Sync>;
type ChildFactory = Box<dyn Fn(Registry, u32) -> Result<Agent, String> + Send + Sync>;

/// The `task` tool: one child agent per call, one prompt, the answer back.
pub struct SubagentTool {
    tools: ToolSource,
    factory: ChildFactory,
    cancel: CancelHandle,
    step_budget: u32,
}

impl SubagentTool {
    /// Build the tool.
    ///
    /// `tools` builds the child's registry from the session's tools; [`TOOL_NAME`]
    /// is removed from it, so a child cannot delegate again. `factory` builds the
    /// child from that registry and the step budget it may use.
    #[must_use]
    pub fn new<T, F>(tools: T, factory: F) -> Self
    where
        T: Fn() -> Registry + Send + Sync + 'static,
        F: Fn(Registry, u32) -> Result<Agent, String> + Send + Sync + 'static,
    {
        Self {
            tools: Box::new(tools),
            factory: Box::new(factory),
            cancel: CancelHandle::new(),
            step_budget: step_budget_from(env::var(STEP_BUDGET_ENV).ok().as_deref()),
        }
    }

    /// Share the session's cancel handle, so Ctrl-C stops the child too.
    #[must_use]
    pub fn with_cancel(mut self, cancel: CancelHandle) -> Self {
        self.cancel = cancel;
        self
    }

    /// Replace the model turns a child may take.
    #[must_use]
    pub fn with_step_budget(mut self, steps: u32) -> Self {
        self.step_budget = steps;
        self
    }
}

impl Tool for SubagentTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }

    fn description(&self) -> &str {
        "Delegate one self-contained job to a child agent and return its answer; \
         the child's exploration stays out of this conversation."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "A 3-6 word label for the job."
                },
                "prompt": {
                    "type": "string",
                    "description": "The complete, self-contained instruction for the child; \
                                    it cannot see this conversation."
                }
            },
            "required": ["description", "prompt"],
            "additionalProperties": false
        })
    }

    async fn run(&self, args: Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        if !args.is_object() {
            return Err(ToolError::InvalidArguments(
                "tool arguments must be a JSON object".to_owned(),
            ));
        }
        let description = required(&args, "description")?.trim().to_owned();
        let prompt = required(&args, "prompt")?;

        let mut tools = (self.tools)();
        tools.remove(TOOL_NAME);
        let mut agent = (self.factory)(tools, self.step_budget)
            .map_err(ToolError::Subagent)?
            .with_history(Vec::new())
            .with_cancel(self.cancel.clone());

        let outcome = agent
            .run(prompt, &mut |_| {})
            .await
            .map_err(|error| ToolError::Subagent(error.to_string()))?;
        Ok(ToolOutput::new(render(
            &description,
            self.step_budget,
            &outcome,
        )))
    }
}

fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    let Some(value) = args.get(key) else {
        return Err(ToolError::InvalidArguments(format!(
            "missing required argument `{key}`"
        )));
    };
    let Some(text) = value.as_str() else {
        return Err(ToolError::InvalidArguments(format!(
            "`{key}` must be a string"
        )));
    };
    if text.trim().is_empty() {
        return Err(ToolError::InvalidArguments(format!(
            "`{key}` must not be empty"
        )));
    }
    Ok(text)
}

fn render(description: &str, step_budget: u32, outcome: &RunOutcome) -> String {
    let mut text = String::new();
    let answer = outcome.text.trim();
    if !answer.is_empty() {
        text.push_str(answer);
        text.push_str("\n\n");
    }
    text.push_str(&format!(
        "subagent: {description} — {} steps, {} tool calls",
        outcome.steps, outcome.tool_calls
    ));
    match &outcome.stop {
        StopReason::Other(reason) if reason == "cancelled" => {
            text.push_str("; cancelled before it finished");
        }
        StopReason::Other(reason) if reason.starts_with("step limit") => {
            text.push_str(&format!("; the step budget of {step_budget} ran out"));
        }
        _ => {}
    }
    text
}

fn step_budget_from(raw: Option<&str>) -> u32 {
    raw.and_then(|value| value.trim().parse().ok())
        .filter(|steps| *steps > 0)
        .unwrap_or(DEFAULT_STEP_BUDGET)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_step_budget_reads_a_positive_integer_and_otherwise_defaults() {
        assert_eq!(step_budget_from(None), DEFAULT_STEP_BUDGET);
        assert_eq!(step_budget_from(Some("5")), 5);
        assert_eq!(step_budget_from(Some(" 7 ")), 7);
        assert_eq!(step_budget_from(Some("0")), DEFAULT_STEP_BUDGET);
        assert_eq!(step_budget_from(Some("many")), DEFAULT_STEP_BUDGET);
    }
}
