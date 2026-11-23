# Upstream Diff

Records edits to upstream-owned files made while evolving Niki.

## Task 4

- `deepagents_code.adapters.runtime.acp.ACPAgentRuntime` now executes the
  checkpointed graph, emits assistant text and approval events, resumes with a
  LangGraph `Command`, and cancels its owned stream task. It does not import or
  depend on `AgentSessionContext`; that type configures the ACP server's graph
  factory and does not identify a live ACP session.
- Production ACP server routing through `RunTurnCoordinator`, plus the TUI and
  headless entrypoint migrations, remain separate work. The current ACP server
  continues to own ACP protocol rendering and session behavior.
