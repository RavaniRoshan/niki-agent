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
- Niki TUI turns now pass through the same managed runtime. A bounded queue
  returns native chunks to the existing Textual processing loop while the
  coordinator validates lifecycle and resume state. Other entry points retain
  direct streaming.
- Niki ACP prompt graphs now pass through `RunTurnCoordinator` inside the
  existing `_AutoGraph` adapter. Native LangGraph chunks still feed the current
  ACP prompt handler, which retains ownership of session IDs, checkpoints,
  permission requests, replay, and protocol rendering. The same runtime turn ID
  and validated event sequence continue through ACP's internal approval resume.
- `dcode` and `deepagents-code` retain the direct ACP graph stream path because
  coordinator routing is enabled only inside `niki_runtime_scope()`.

## Task 5

- The server runtime now exposes a `WorkspacePort` adapter over the existing
  composite backend. Each workspace runtime binds that adapter to the validated
  workspace ID and root before use; file and command operations reject paths
  outside that root, including symlink escapes.
- The backend adapter uses `aread`, `awrite`, and `aexecute`, translates their
  result types, surfaces backend failures, and safely quotes shell arguments.
- `DefaultApprovalPolicy` now translates the live approval mode used by the
  existing `interrupt_on` predicate. It does not infer permissions from tool
  name prefixes or introduce a separate approval-mode store.
- The `ServerRuntime` tuple gained an optional trailing workspace port field;
  existing three-argument construction remains compatible.
