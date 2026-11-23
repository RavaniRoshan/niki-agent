# Niki Architecture

## Dependency direction

Inner layers are framework-free. Outer adapters depend on inner contracts, never the reverse.

```
Entry points (TUI, headless, ACP)
  -> Application use cases (RunTurnCoordinator, ports)
    -> Domain (RunEvent, SessionSummary, TurnStatus, ...)
  <- Adapters (LangGraph, workspace, models, MCP, skills, hooks, memory)
```

- `deepagents_code.domain` and `deepagents_code.application` import only the standard library.
- Adapters live under `deepagents_code.adapters/` and implement the port protocols.
- Framework objects (LangGraph, LangChain, Textual, Rich, Deep Agents) stay inside adapters.

## Entry-point ownership

- Client owns input, approvals, and presentation (TUI, headless, ACP surfaces).
- Managed server owns graph execution, tools, workspace policy, and checkpoints.
- ACP remains a distinct adapter until its behavior is covered by shared contracts.

## LangGraph vs LangChain

- LangGraph is the current graph/runtime engine (managed server, checkpointers, graph execution).
- LangChain integrations are used for model providers.
- Both are kept behind outer adapters; domain and application code never import either.

## Event contract

`RunEvent` is the single envelope streamed across all surfaces. `schema_version` is 1.
`sequence` starts at 1 per turn and continues across approval/resume pauses.
`encode_run_event` rejects envelopes larger than 65,536 UTF-8 bytes.
