# Upstream Diff

Records edits to upstream-owned files made while evolving Niki.

## Task 4

- `deepagents_code.adapters.runtime.acp.ACPAgentRuntime` now executes the
  checkpointed graph, emits assistant text and approval events, resumes with a
  LangGraph `Command`, and cancels its owned stream task. It does not import or
  depend on `AgentSessionContext`; that type configures the ACP server's graph
  factory and does not identify a live ACP session.
- Niki headless turns now pass through `RunTurnCoordinator` and
  `LangGraphRemoteRuntime`. The adapter forwards native chunks to the existing
  headless renderer, preserving its output and hook behavior. The legacy
  `dcode` path remains on its existing stream path.
- The TUI migration and production ACP server routing remain separate work.
  The current ACP server continues to own ACP protocol rendering and session
  behavior; the ACP runtime adapter is not yet wired into that server.
