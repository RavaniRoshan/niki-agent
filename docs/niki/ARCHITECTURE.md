# Niki Architecture

## Dependency direction

Inner layers are framework-free. Outer adapters depend on inner contracts, never the reverse.

```
Entry points (TUI, headless, ACP)
  -> Application use cases (RunTurnCoordinator, ports)
    -> Domain (RunEvent, SessionSummary, TurnStatus, ...)
  <- Runtime adapters (LangGraph remote stream, ACP graph)

Managed server composition root (server_graph.py)
  -> LangGraph graph + provider models + tools + MCP + workspace policy
```

- `deepagents_code.domain` and `deepagents_code.application` import only the standard library.
- `AgentRuntime` is the active application boundary for complete streamed turns; its adapters live under `deepagents_code.adapters.runtime/`.
- `server_graph.py` is the managed server composition root. It owns graph execution and composes the current provider, tool, MCP, workspace, and checkpoint integrations.
- Tool invocation and approval stay together on the server side; a client-side tool registry must not bypass that policy boundary.
- Provider execution stays inside the LangGraph runtime so streamed text, tool calls, retries, usage metadata, and checkpoint resumes share one path.
- Textual presentation stays in client adapters and widgets. Provider, MCP, skill, and hook integrations do not import Textual UI classes.
- Do not add separate model-generation or tool-registry ports until a concrete consumer can use them without duplicating the active runtime or losing its event and permission behavior.
- Framework objects (LangGraph, LangChain, Textual, Rich, Deep Agents) stay outside the domain and application layers.

## Entry-point ownership

- Client owns input, approvals, and presentation (TUI, headless, ACP surfaces).
- Managed server owns graph execution, tools, workspace policy, and checkpoints.
- ACP remains a distinct adapter until its behavior is covered by shared contracts.

## LangGraph vs LangChain

- LangGraph is the current graph/runtime engine (managed server, checkpointers, graph execution).
- LangChain integrations are used for model providers.
- Both are composed by the managed server boundary; domain and application code never import either.

## Event contract

`RunEvent` is the single envelope streamed across all surfaces. `schema_version` is 1.
`sequence` starts at 1 per turn and continues across approval/resume pauses.
`encode_run_event` rejects envelopes larger than 65,536 UTF-8 bytes.
