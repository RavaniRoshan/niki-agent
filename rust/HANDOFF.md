# Rust client status

The Rust workspace in `rust/` is a second front end for the same engine, built
one crate at a time. Python is still the product until the port reaches its
cutover wave: see [`MAP.md`](./MAP.md) for the wave plan and
[`DECISIONS.md`](./DECISIONS.md) for the reasoning behind it. W0–W3 are done.
W4 (extensions) has started: `niki-mcp` speaks stdio and HTTP, parses
`[mcp_servers]`, registers discovered tools in the existing registry, and both
`niki-rs` entry points now connect the configured servers at startup.
`niki-skills` discovers project, user and built-in skills and both entry points
give them to the model through the system prompt. `niki-hooks` runs
`PreToolUse`, `PostToolUse` and `UserPromptSubmit` handlers from `hooks.json`,
and `niki-commands` owns the slash command grammar. The screen persists every
completed turn and handles bracketed paste.

## What is implemented

| Crate | What it answers from Rust |
| --- | --- |
| `niki-config` | `config.toml` parsing: provider blocks, `[models].default`, `[ui].theme`, `[mcp_servers.<name>]` |
| `niki-provider` | one streaming turn against OpenAI-compatible or Anthropic endpoints |
| `niki-tools` | read/write/edit/list, shell, grep, glob over a rooted workspace; runtime tool registration |
| `niki-policy` | Manual/Auto/YOLO gate, shell allow-list, path boundary |
| `niki-agent` | the turn loop, event callback, approval callback, step budget, cancel |
| `niki-mcp` | MCP handshake, `tools/list`, `tools/call` over stdio and HTTP; `Tool` bridges, loaded at startup by both entry points under a per-server startup budget |
| `niki-skills` | project/user/built-in skill discovery, front-matter parsing, system-prompt injection, `/skills` |
| `niki-hooks` | `hooks.json` loading, matcher selection, command handlers, blocking verdicts; installed by both `niki-rs` entry points |
| `niki-workspace` | root detection, ignore rules, file index |
| `niki-session` | SQLite transcript store, resume, checkpoints |
| `niki-render` | transcript view model, deterministic frame, `wrap` |
| `niki-tui` | keymap (`BINDINGS`, `chord_label`), reader mode, view state |
| `niki-tui-themes` | four palettes, WCAG AA contrast gate, glyph and motion modes |
| `niki-tui-modals` | approval prompt, model picker, help overlay with the command list |
| `niki-tui-live` | the full-screen app: composer with bracketed paste, transcript, status bar, worker, session persistence |
| `niki-commands` | slash command grammar (`/clear`, `/compact`, `/resume`, `/tools`, `/version` beside the existing four) |
| `niki` | `niki` (engine client) and `niki-rs` (headless and full-screen) |

`niki-rs -n "prompt"` is the headless entry point. `niki-rs` with no `-n` opens
the full-screen app when stdout is a terminal. `niki` still replays a protocol
fixture and streams from the Python engine; it is unchanged.

Both entry points read `[mcp_servers.<name>]` from the config and connect every
server before the first turn. Their tools are offered to the model as
`mcp__<server>__<tool>`, in the same registry as the built-ins. A mutating tool —
one without a coherent `readOnlyHint` — takes the approval prompt in the screen;
a headless run has nobody to ask, so it reports the request to the model as a
refusal. A server that fails to start does not stop the session: the screen
shows `warning: mcp server …` as a transcript notice before the first frame and
the run continues with the remaining servers (headless prints the same warning
on stderr).

Skills are discovered once at startup from `<root>/.niki/skills`,
`<NIKI_HOME or ~/.niki>/skills` and the two built-in skills compiled into
`niki-skills` (`commit`, `review`), with the first match for a name winning in
that order. Both entry points inject a compact `## Skills` section into the
system prompt — name, description, and for a skill on disk the `SKILL.md` path
to read when it applies — so a long body costs nothing on a turn that does not
use it; a built-in has no file, so its short instructions are the one body the
section carries. A `SKILL.md` with no description, or front matter that does not
parse, is skipped with a warning and the session continues (headless prints the
warnings on stderr); the screen shows the whole catalog, with paths and skipped
files, through `/skills`.

Hooks run beside those extensions. `niki-hooks` reads
`{workspace}/.niki/hooks.json` and `{config_home}/hooks.json` (the project file
wins for an event it defines, the user file supplies the rest), parses the
Python client's `{"hooks": {<event>: [{matcher, hooks: [{type, command,
timeout}]}]}}` shape, and maps built-in tool names and arguments to the wire
vocabulary (`shell` → `Bash`, `write_file` → `Write`, `edit_file` → `Edit`, …)
so an existing `hooks.json` keeps selecting the same tool. Each handler is a
`/bin/sh -c` child with one JSON payload on stdin (`hook_event_name`, `cwd`,
`session_id` when the host has one, `tool_name`, `tool_input`, `tool_use_id`;
`tool_response` and `duration_ms` on `PostToolUse`; `prompt` on
`UserPromptSubmit`). Exit code 2, JSON `{"decision":"block"}` and
`permissionDecision: "deny"` block `PreToolUse`; the model receives
`<tool> blocked by hook: <reason>` as the tool result, exactly the Python
wording. A blocked `UserPromptSubmit` returns `AgentError::HookBlocked` before
any model request, and a `PostToolUse` block is feedback appended to the
result. Handlers run in configuration order with a 600 s default timeout (30 s
for prompts); a command that cannot start, times out or writes unusable output
is a warning notice and the turn continues. Both `niki-rs` entry points install
the runner: headless prints load warnings to stderr, and the full-screen app
adds one warning row per broken source before the first frame.
`permissionDecision` `allow`/`ask`/`defer`, `updatedInput`,
`suppressOriginalPrompt` and `PostToolUseFailure` are not implemented and are
reported as warnings; project-hook trust prompting is not ported either, so
loading `.niki/hooks.json` is not gated yet.

The shell surface closes the two M3 gaps: an interactive `niki-rs` run
opens `~/.niki/sessions.db`, creates a session row immediately and appends every
completed turn as `user`, `assistant`, `tool_call`, `tool_result`, `tool_error`
or `error` messages, so `/resume` lists and rebuilds sessions on the next
launch. `/clear` starts a new session without deleting the old one, `/compact`
folds older rows out of the view only — this build has no summarising model, and
the help overlay says so — `/tools` lists the registry including `mcp__` names,
and `/version` names Niki Agent. Bracketed paste is enabled with the alternate
screen, arrives as one `Event::Paste`, and is inserted literally; a paste never
sends the prompt, and a second prompt waits for the running turn so a turn can
never be written to the store under another turn's rows. MCP startup is now
concurrent and abandons a silent server after `NIKI_MCP_STARTUP_TIMEOUT_SECS`
(default 10s), reporting the same per-server warning.

## Gates

```
cd rust
cargo fmt --all
cargo clippy -j 2 --workspace --all-targets -- -D warnings
cargo test -j 1 --workspace        # 324 tests, 0 failures
```

`crates/niki/tests/niki_rs.rs` is the W1 exit gate: it binds a mock OpenAI SSE
server on loopback, runs the binary as a child process, and asserts the file the
model asked for exists and that the tool result travelled back in the second
request body. W3 kept it green without touching it.

The W3 gates need no terminal:
`crates/niki-tui-live/tests/screen.rs` renders into a ratatui `TestBackend`
buffer and reads the cells back; `tests/turn.rs` runs real turns through the
worker against a scripted provider and drives approvals end to end.
`crates/niki-mcp/tests/wiring.rs` drives the stdio fixture through
`niki_mcp::load`, a real `Agent::run` approval and the screen's worker, so the
startup wiring is covered without a network or a terminal.

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
  which is what Esc and Ctrl-C rely on. When the host opened I/O first — the MCP
  connections — it hands that same runtime to
  `WorkerHandle::spawn_with_runtime`: a tokio resource belongs to the runtime
  that opened it, so the turns must run there too. The MCP servers are connected
  before the worker starts, which is why their warnings are transcript notices
  from the first frame.
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

## Next: W5 — ACP and headless parity

W4 is complete: `niki-mcp` (stdio, HTTP, config, registration, startup
wiring and a per-server startup budget), `niki-skills` (discovery, front
matter, system-prompt injection, `/skills`) and `niki-hooks` are done, and
`niki-commands` owns `App::slash` with the help overlay reading the same
table. Both entry points call `niki_mcp::load` and install the hooks runner.
W5 (ACP and headless parity) is next.

## Known gaps

- MCP startup now bounds the handshake, but a later request still uses the fixed
  one-minute budget; a per-server `timeout` key is the next thing to add.
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
- The composer has no mouse selection.
- `Ctrl+O`, `Ctrl+T`, `Ctrl+N`, `Ctrl+G` are in `BINDINGS` but not implemented in
  the live app; the help overlay says so.
- ratatui 0.29 pulls crossterm 0.28 alongside the workspace's 0.29. Both are
  small and the ANSI output agrees, but a single version would be tidier at
  cutover.
- `niki-engine-client` and `niki-proto` still describe themselves in terms of the
  runtime the Python engine speaks; their crate descriptions need a product-level
  rewrite when the engine is retired (W6).
- A user skill's `SKILL.md` lives outside the workspace root, and `read_file`
  refuses a path outside the root (RULES 7). The model is told about every
  skill, and reads project skills (`.niki/skills`, inside the root) normally;
  opening a user skill needs a read path that does not exist yet — a mount in
  `niki-tools` or a dedicated skill tool. Built-in skills are unaffected
  because their instructions are injected.
- User skills are read from `<NIKI_HOME or ~/.niki>/skills` in this slice. The
  Python client keeps profile skills at `~/.niki/<profile>/skills` (default
  `~/.niki/agent/skills`), so the two clients do not see the same user skills
  yet; the profile layout is a W4/W6 decision.
- Token counting is provider-reported only (ADR-005 open question 1).
