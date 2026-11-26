<!--
  Mirrored from docs/niki/ARCHITECTURE.md by docs-site/scripts/sync-docs.mjs.
  Do not edit this copy: edit the source file and run `npm run sync`.
  source-sha256: 6e2f9256370cc243589866895a08c4c605b60c5b695b534eef460fa2ec8d093d
-->
# How Niki Agent fits together

Niki is one Python package, `libs/code/niki_agent`, with a small set of internal
boundaries. The boundaries exist so the terminal UI, the headless mode, and the
editor-protocol adapter can share one agent loop without each of them growing its
own copy.

## The shape of a turn

```
Entry points (TUI, headless, ACP)
  -> Application layer (run coordinator, ports)
    -> Domain (run events, sessions, turn status)
  <- Runtime adapters (agent stream, ACP)

Server composition (server_graph.py)
  -> agent graph + provider model + tools + MCP + workspace policy
```

A request enters through one of the entry points, the application layer
coordinates the turn, and the runtime adapter streams events back. Both the
terminal client and the headless client consume the same event stream, so a
behavior added to the loop reaches every surface.

## Layers

- `niki_agent.domain` and `niki_agent.application` import only the standard
  library. They define the events and the turn contract, not the machinery that
  produces them.
- `AgentRuntime` (`niki_agent/application/run_turn.py`) is the application
  boundary for a complete streamed turn; its adapters live under
  `niki_agent/adapters/runtime/`.
- `server_graph.py` is the composition root. It owns graph execution and wires
  the active provider, tools, MCP servers, workspace policy, and checkpoints
  together.
- Tool invocation and approval stay on the same side of the boundary. A client
  cannot register a tool that bypasses the approval policy.
- Provider execution stays inside the graph, so streamed text, tool calls,
  retries, usage accounting, and checkpoint resumes share one path.
- Presentation stays in Textual widgets and client adapters. Provider, MCP,
  skill, and hook integrations never import UI classes.

## Entry points

| Surface | Owns |
| --- | --- |
| TUI (`niki`) | Input, approvals, the transcript, and presentation. |
| Headless (`niki -n`) | One turn, plain output on stdout. |
| ACP (`niki_agent.acp`) | The editor-protocol adapter for editors that speak ACP. |

The client owns input, approvals, and presentation. The server side owns graph
execution, tools, workspace policy, and checkpoints. ACP stays a separate
adapter until its behavior is expressed in the shared contracts.

## The event contract

`RunEvent` is the single envelope between the runtime and every Python client
surface, defined in `niki_agent/domain/events.py`:

- `schema_version` is `1`. An event with another version is rejected, not
  guessed at.
- `sequence` starts at 1 for each turn and keeps counting across an
  approval/resume pause.
- `encode_run_event` rejects an envelope larger than 65,536 UTF-8 bytes, so a
  single malformed tool result cannot stall a stream.

The TUI, headless mode, and ACP adapter all read this envelope. The Rust client
in `rust/` speaks the engine's HTTP/SSE protocol directly instead; its wire
vocabulary is defined in `rust/crates/niki-proto`.

---

Source: [`docs/niki/ARCHITECTURE.md`](https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/docs/niki/ARCHITECTURE.md) in the Niki Agent repository.
