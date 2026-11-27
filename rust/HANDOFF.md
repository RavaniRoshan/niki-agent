# Rust client status

The Rust workspace in `rust/` is a second front end for the same engine, built
one crate at a time. Python is still the product until the port reaches its
cutover wave: see [`MAP.md`](./MAP.md) for the wave plan and
[`DECISIONS.md`](./DECISIONS.md) for the reasoning behind it. W0–W3 are done.
W4 (extensions) has started: `niki-mcp` speaks stdio and HTTP, parses
`[mcp_servers]`, registers discovered tools in the existing registry, and the
`niki` entry points now connect the configured servers at startup.
`niki-skills` discovers project, user and built-in skills and every entry point
gives them to the model through the system prompt. `niki-hooks` runs
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
| `niki-mcp` | MCP handshake, `tools/list`, `tools/call` over stdio and HTTP; `Tool` bridges, loaded at startup by every entry point under a per-server startup budget |
| `niki-skills` | project/user/built-in skill discovery, front-matter parsing, system-prompt injection, `/skills` |
| `niki-hooks` | `hooks.json` loading, matcher selection, command handlers, blocking verdicts; installed by every `niki` entry point |
| `niki-workspace` | root detection, ignore rules, file index |
| `niki-session` | SQLite transcript store, resume, checkpoints |
| `niki-render` | transcript view model, deterministic frame, `wrap` |
| `niki-tui` | keymap (`BINDINGS`, `chord_label`), reader mode, view state |
| `niki-tui-themes` | four palettes, WCAG AA contrast gate, glyph and motion modes |
| `niki-tui-modals` | approval prompt, model picker, help overlay with the command list |
| `niki-tui-live` | the full-screen app: composer with bracketed paste, transcript, status bar, worker, session persistence |
| `niki-commands` | slash command grammar (`/clear`, `/compact`, `/resume`, `/tools`, `/version` beside the existing four) |
| `niki-acp` | a server for the Agent Client Protocol on stdin/stdout: initialize, session/new, session/load, session/prompt, session/cancel, session/set_mode, session/set_config_option, and `session/update` notifications |
| `niki` | `niki` (engine client) and `niki` (headless, full-screen and `--acp`) |

`niki -n "prompt"` is the headless entry point. `niki` with no `-n` opens
the full-screen app when stdout is a terminal. `niki` still replays a protocol
fixture and streams from the Python engine; it is unchanged.

Every entry point reads `[mcp_servers.<name>]` from the config and connects
every server before the first turn. Their tools are offered to the model as
`mcp__<server>__<tool>`, in the same registry as the built-ins. A mutating tool —
one without a coherent `readOnlyHint` — takes the approval prompt in the screen;
a headless run has nobody to ask, so it reports the request to the model as a
refusal. A server that fails to start does not stop the session: the screen
shows `warning: mcp server …` as a transcript notice before the first frame and
the run continues with the remaining servers (headless prints the same warning
on stderr).

Skills are discovered from `<root>/.niki/skills`, `<NIKI_HOME or ~/.niki>/skills`
and the two built-in skills compiled into `niki-skills` (`commit`, `review`),
with the first match for a name winning in that order. The screen and a headless
run discover once at startup; the ACP server discovers for each session root.
Every entry point injects a compact `## Skills` section into the system prompt —
name, description, and for a skill on disk the `SKILL.md` path to read when it
applies — so a long body costs nothing on a turn that does not use it; a
built-in has no file, so its short instructions are the one body the section
carries. A `SKILL.md` with no description, or front matter that does not parse,
is skipped with a warning and the session continues (headless and the ACP server
print the warnings on stderr); the screen shows the whole catalog, with paths
and skipped files, through `/skills`.

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
is a warning notice and the turn continues. Every `niki` entry point installs
the runner: headless and the ACP server print load warnings to stderr (the
server loads hooks for each session root), and the full-screen app adds one
warning row per broken source before the first frame. The ACP server shows a
hook's warning or `systemMessage` as an `agent_message_chunk` update.
`permissionDecision` `allow`/`ask`/`defer`, `updatedInput`,
`suppressOriginalPrompt` and `PostToolUseFailure` are not implemented and are
reported as warnings; project-hook trust prompting is not ported either, so
loading `.niki/hooks.json` is not gated yet.

The shell surface closes the two M3 gaps: an interactive `niki` run
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

`niki --acp` serves an editor on stdin/stdout; `--acp` with `-n` is a
usage error and exits 2. The protocol is newline-delimited JSON-RPC 2.0 — one
compact object per line, no `Content-Length` header — as the reference
implementation frames it. The server answers `initialize`, `session/new`,
`session/load`, `session/prompt`, `session/set_mode` and
`session/set_config_option`, takes `session/cancel` as a notification, and
streams `session/update` with `agent_message_chunk`, `tool_call`,
`tool_call_update` and `plan` updates. A prompt runs the same `Agent::run` as
the other entry points, rooted at the `cwd` of `session/new`. Sessions persist
in `~/.niki/sessions.db` with the screen's roles, so `session/load` replays a
transcript and either surface can read the other's sessions.

## W5 decisions

- **Framing and methods are copied, not guessed.** The installed `acp`
  package (schema-v1.16.0) was read for the transport (newline-delimited
  frames), the method names, every request/response alias (`sessionId`,
  `stopReason`, `toolCallId`, `rawInput`, `sessionUpdate`) and the error
  codes. `-32700` is never sent because the reference server drops a malformed
  line and keeps reading; this server does the same.
- **Approval is the existing policy, surfaced as the protocol's permission
  request.** `Decision::Ask` becomes `session/request_permission` with
  `allow_once`, `reject_once` and `allow_always` options; the answer maps to
  `Approval::Once`, `Always` or `Deny`, and a cancelled outcome or an
  unreachable client is `Deny`. The loop reports a call's name and arguments on
  start and only the name and output on end, so the server keeps the started
  calls and matches an end to the oldest of that name; the executing call is
  what the permission prompt describes. `Always` on a shell command adds the
  exact command to the existing allow-list, which is exact/prefix matching —
  the Python side records command *types* instead.
- **Only the mode is a config option.** `Agent` exposes no message-seeding or
  model setter, so rebuilding an agent for a new model would drop the model's
  conversation. Advertising a model selector would promise context the loop
  cannot keep; `session/set_config_option` with `model` answers `-32602`
  ("Unknown config option"), as an option the server never offered.
- **Plan updates are driven by a tool call, as in the reference server.** A
  registered `write_todos`/`todo_write` call's arguments become `plan` entries
  (unknown statuses fold to `pending`, priority `medium`) right after its
  `tool_call` update, live and on replay. The default registry has no todo tool,
  so a default session emits none; the mapping is covered with a registered
  tool.
- **Replay is atomic.** `session/load` reads the whole transcript first, then
  emits it, then answers: a write cannot tear the replay. Rows map to
  `user_message_chunk`, `agent_message_chunk`, `tool_call` and
  `tool_call_update`, with the store's row id as `messageId`.
- **Session ids are the store's ids when a store is open.** Without one the
  server mints a timestamp id and does not advertise `loadSession`, which is the
  reference server's condition too.

## Gates

```
cd rust
cargo fmt --all
cargo clippy -j 2 --workspace --all-targets -- -D warnings
cargo test -j 1 --workspace        # 356 tests, 0 failures
```

`crates/niki-acp` contributes 27 of them: 14 unit tests for framing, the
protocol shapes, the plan mapping and the stop-reason mapping, and 13
integration tests that drive `serve` over in-memory pipes with a scripted
provider — initialize, a live prompt, a tool call, a permission round-trip
(always-allow and reject), cancel, load and its error cases, mode and config
changes, plan updates, a provider failure naming the provider, a half-closed
client still receiving its answers, and a malformed line. No test opens a
socket.

`crates/niki/tests/niki_rs.rs` is the W1 exit gate: it binds a mock OpenAI SSE
server on loopback, runs the binary as a child process, and asserts the file the
model asked for exists and that the tool result travelled back in the second
request body. W3 kept it green without touching it.

The screen draws its first frame, then only when a worker event, a spinner
tick while a turn is busy, or an input event changes something. An idle poll
waits 60 s and paints nothing when it times out, so a pty sitting open writes
no bytes; typing still repaints. Keep the claim tied to that loop.

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
- The binary is `niki` until W6 (RULES.md 2).

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

## W5 — ACP and headless parity

`niki-acp` serves an editor on stdin/stdout (`--acp`): `initialize`,
`session/new`, `session/prompt`, cancel, mode and config, with the agent's
events streamed as session updates. A gated command becomes `session/request_permission`;
a client that cannot be reached is a denial, not a hang.

The headless entry point now matches the Python client's conventions:

- `--output-format json` prints one object after the turn: `answer`, `steps`,
  `tool_calls`, `usage`, `stop` (`end_turn`, `tool_use`, `max_tokens`,
  `step_limit`, `cancelled`, `other`) and `session`. A script parses that
  instead of scraping text; errors still go to stderr.
- `--continue` reopens the newest session for the workspace and
  `--resume <id|last>` a named one. Its `user` and `assistant` rows seed the
  conversation; `tool_call`/`tool_result` rows stay in the store but are not
  resent, because a stored row has no tool-call id to answer. A headless run
  also appends its own prompt, tool rows and answer, so the screen's `/resume`
  lists it.
- Exit 124 when the step budget runs out (GNU `timeout`'s code, which the
  Python client uses), 130 when Ctrl-C cancels, 2 for a usage error such as
  `--acp -n`, 1 for a failure, 0 otherwise.

The provider stream is read to its end rather than stopping at the finish
reason, because OpenAI sends the usage chunk after it; token counts were
always zero before that change.

W4 is complete: `niki-mcp` (stdio, HTTP, config, registration, startup
wiring and a per-server startup budget), `niki-skills` (discovery, front
matter, system-prompt injection, `/skills`) and `niki-hooks` are done, and
`niki-commands` owns `App::slash` with the help overlay reading the same
table. Every entry point calls `niki_mcp::load` and installs the hooks runner,
and all three inject the same skills section into their system prompt. W5 (ACP
and headless parity) has started: `niki-acp` is done; headless parity remains.

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
- **ACP:** `agent_thought_chunk` is never emitted. The turn loop's event
  stream has text, tool-call starts/ends, usage, a step limit and a cancel — no
  reasoning channel — so a thought would have to be invented. The Python server
  emits them from the model runtime's reasoning blocks; the Rust provider
  normalises none.
- **ACP:** images and audio are refused with `-32602` and
  `promptCapabilities.image=false`; the message model carries text only.
  `resource_link` and embedded text/blob blocks become text metadata, as the
  Python adapter does.
- **ACP:** `mcpServers` in `session/new` and `session/load` is accepted and
  ignored. Tools come from `config.toml` at startup; per-session MCP lifecycle
  and the protocol's `acp` MCP transport are not implemented.
- **ACP:** `session/load` replays the transcript but the model's context
  restarts — the turn loop exposes no way to seed `Agent::messages`, so the
  resumed session answers without earlier turns in its request.
- **ACP:** `additionalDirectories` is ignored; `cwd` is the only root. Mode is
  not persisted (the store has no metadata column), so a loaded session starts
  in the process default. A second prompt while one runs is refused (`-32603`)
  rather than queued, and `set_mode` waits for the running turn.
- Token counting is provider-reported only (ADR-005 open question 1).
