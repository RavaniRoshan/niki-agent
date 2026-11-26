# Rust client status

The Rust workspace in `rust/` is a second front end for the same engine, built
one crate at a time. Python is still the product until the port reaches its
cutover wave: see [`MAP.md`](./MAP.md) for the wave plan and
[`DECISIONS.md`](./DECISIONS.md) for the reasoning behind it. W0–W3 are done.
W4 (extensions) has started: `niki-mcp` speaks stdio and HTTP, parses
`[mcp_servers]`, and registers discovered tools in the existing registry.
`niki-skills`, `niki-hooks` and `niki-commands` are next.

## What is implemented

| Crate | What it answers from Rust |
| --- | --- |
| `niki-config` | `config.toml` parsing: provider blocks, `[models].default`, `[ui].theme`, `[mcp_servers.<name>]` |
| `niki-provider` | one streaming turn against OpenAI-compatible or Anthropic endpoints |
| `niki-tools` | read/write/edit/list, shell, grep, glob over a rooted workspace; runtime tool registration |
| `niki-policy` | Manual/Auto/YOLO gate, shell allow-list, path boundary |
| `niki-agent` | the turn loop, event callback, approval callback, step budget, cancel |
| `niki-mcp` | MCP handshake, `tools/list`, `tools/call` over stdio and HTTP; `Tool` bridges |
| `niki-workspace` | root detection, ignore rules, file index |
| `niki-session` | SQLite transcript store, resume, checkpoints |
| `niki-render` | transcript view model, deterministic frame, `wrap` |
| `niki-tui` | keymap (`BINDINGS`, `chord_label`), reader mode, view state |
| `niki-tui-themes` | four palettes, WCAG AA contrast gate, glyph and motion modes |
| `niki-tui-modals` | approval prompt, model picker, help overlay |
| `niki-tui-live` | the full-screen app: composer, transcript, status bar, worker |
| `niki` | `niki` (engine client) and `niki-rs` (headless and full-screen) |

`niki-rs -n "prompt"` is the headless entry point. `niki-rs` with no `-n` opens
the full-screen app when stdout is a terminal. `niki` still replays a protocol
fixture and streams from the Python engine; it is unchanged.

## Gates

```
cd rust
cargo fmt --all
cargo clippy -j 2 --workspace --all-targets -- -D warnings
cargo test -j 2 --workspace        # 227 tests, 0 failures
```

`crates/niki/tests/niki_rs.rs` is the W1 exit gate: it binds a mock OpenAI SSE
server on loopback, runs the binary as a child process, and asserts the file the
model asked for exists and that the tool result travelled back in the second
request body. W3 kept it green without touching it.

The W3 gates need no terminal:
`crates/niki-tui-live/tests/screen.rs` renders into a ratatui `TestBackend`
buffer and reads the cells back; `tests/turn.rs` runs real turns through the
worker against a scripted provider and drives approvals end to end.

## Decisions a contributor must know

- The agent assembles tool calls from `StreamEvent::ToolCallDelta` itself, using
  `niki_provider::openai::ToolCallAssembler`. The adapters emit fragments; nobody
  else may assume whole calls.
- A tool failure and a policy refusal are **tool results**, not `AgentError`s, so
  the model can react. Only provider failures abort a run.
- `Decision::Ask` goes to the `Approver` installed with `Agent::with_approver`:
  `Fn(ApprovalRequest) -> BoxFuture<'static, Approval>` answering `Once`,
  `Always` (which grows the session allow-list through `AllowList::add`) or
  `Deny`. With no approver — a headless run — the request is still reported to
  the model as a refusal.
- The turn runs on a worker thread with its own current-thread tokio runtime.
  The screen talks to it over `std::sync::mpsc` and answers approvals over a
  `tokio::sync::oneshot`; dropping that sender unblocks the turn with `Deny`,
  which is what Esc and Ctrl-C rely on.
- One `CancelHandle` serves the session; the worker calls `reset()` before each
  turn, so a cancel always applies to the turn in flight.
- Keyboard ownership: the composer claims characters, arrows, Home/End, Enter
  (send) and Alt+Enter (newline). The app claims Ctrl-C, Ctrl-D, PageUp/PageDown,
  Shift+Tab and the wheel. A modal claims everything while it is open, except
  Ctrl-C and Ctrl-D. The help overlay lists `BINDINGS`; the last four entries in
  `describe()` say "not in this build" because the live app does not act on them
  yet — keep that marker honest rather than claiming a key works.
- `[ui].theme` picks the palette (`niki`, `high-contrast`, `colorblind`, `mono`,
  plus the aliases in `niki_tui_themes::palette`); `/theme` cycles and
  `/theme <name>` selects. `NIKI_ASCII=1` and `NIKI_REDUCED_MOTION=1` do what
  they say at startup.
- The binary is `niki-rs` until W6 (RULES.md 2).

## M4 decisions

- **Config types live in `niki-config`.** `McpServer` / `McpTransport` are
  parsed there, so `niki-config` stays free of the JSON-RPC and process
  implementation and `niki-mcp` depends on the shape rather than the other way
  round. A malformed entry is `ConfigError::McpServer { server, .. }`, which
  names the table the way provider failures name the file.
- **Tool names are `mcp__<server>__<tool>`.** That is the name the Python
  client uses in hooks and matchers (`format_mcp_wire_name`), so a saved matcher
  carries over. The Python client's *provider-visible* name is
  `{server}_{tool}`; the two differ, and this slice follows the wire form the
  milestone names. Characters outside `[A-Za-z0-9_-]` become `_`, and a name
  longer than the provider cap keeps an FNV-1a digest instead of SHA-256 (the
  Python client's hash); only determinism matters here.
- **A mutating MCP tool is gated; a built-in write is not (yet).** In this build
  `write_file` runs without asking — the W1 headless gate depends on it — so
  "the same approval path as a built-in write" cannot mean an existing write
  prompt. The conservative reading is implemented instead: `Tool::requires_approval`
  defaults to `false`, MCP bridges return `!readOnlyHint`, and the agent sends a
  gated tool through the same `Approver` a shell command uses. `Always` records
  the tool name in the agent, not in the shell allow-list. Auto asks as Manual
  does because this client has no classifier; YOLO runs it. When built-in writes
  are gated (a later slice), they can take the same flag.
- **HTTP is streamable HTTP, not legacy SSE.** A POST is answered with a JSON
  body or an SSE body; both are parsed, and a stream that never closes fails as
  a timeout. `type = "sse"` (the long-lived GET transport) is rejected at load
  with a message naming the server. The one-minute request budget is fixed for
  this slice; a per-server `timeout` key is the next thing to add.
- **One request at a time per server.** stdio responses are matched by id but a
  lost response cannot be resynchronised, so the connection is serialised behind
  one async mutex and marked closed after a timeout, exit or malformed line.
  HTTP requests are independent and are not marked closed.

## Next: W4 — extensions

`niki-mcp` is done for stdio, HTTP, config and registration. It is not wired
into `niki-rs` yet: the binary still builds `Registry::new()` and never calls
`niki_mcp::load(&config.mcp_servers)`. Doing that in `headless` is easy (the
runtime is already there); the screen's worker builds the agent in a synchronous
closure, so it needs a runtime handle or a startup load before the worker
starts. That is the next slice, together with `niki-skills`, `niki-hooks` and
`niki-commands`. Slash commands are currently four hard-coded strings in
`App::slash`; W4 should move them into `niki-commands` and have `niki-tui-live`
call it, keeping `/quit`, `/help`, `/model` and `/theme` as the built-ins.

## Known gaps

- `niki-mcp` is not called by either entry point yet (see above).
- MCP: no OAuth (the Python client's `auth = "oauth"` has no Rust counterpart),
  no `${VAR}` interpolation in `command`/`args`/`env`/`url`/`headers`, no
  `tools/list` pagination (`nextCursor`), and no `DELETE` to end an HTTP session.
- MCP HTTP has no integration test: a test server would need a listener. The SSE
  parser, batch matching, server-request replies and JSON-RPC error decoding are
  unit-tested; the six required protocol cases run against the stdio fixture.
- Built-in writes — `write_file`, `edit_file` — are not approval-gated, unlike
  the Python product. This slice deliberately did not change that (see M4
  decisions).
- No streaming cancel inside a provider request beyond the per-event check.
- The live app has no session persistence yet: `niki-session` exists and is
  tested, but the screen does not write to it, so a restart loses the transcript.
- The composer does not handle bracketed paste or a mouse selection.
- `Ctrl+O`, `Ctrl+T`, `Ctrl+N`, `Ctrl+G` are in `BINDINGS` but not implemented in
  the live app; the help overlay says so.
- ratatui 0.29 pulls crossterm 0.28 alongside the workspace's 0.29. Both are
  small and the ANSI output agrees, but a single version would be tidier at
  cutover.
- `niki-engine-client` and `niki-proto` still describe themselves in terms of the
  runtime the Python engine speaks; their crate descriptions need a product-level
  rewrite when the engine is retired (W6).
- Token counting is provider-reported only (ADR-005 open question 1).
