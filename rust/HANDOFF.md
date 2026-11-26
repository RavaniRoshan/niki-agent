# Rust client status

The Rust workspace in `rust/` is a second front end for the same engine, built
one crate at a time. Python is still the product until the port reaches its
cutover wave: see [`MAP.md`](./MAP.md) for the wave plan and
[`DECISIONS.md`](./DECISIONS.md) for the reasoning behind it. W0–W3 are done;
W4 (extensions) is next.

## What is implemented

| Crate | What it answers from Rust |
| --- | --- |
| `niki-config` | `config.toml` parsing: provider blocks, `[models].default`, `[ui].theme` |
| `niki-provider` | one streaming turn against OpenAI-compatible or Anthropic endpoints |
| `niki-tools` | read/write/edit/list, shell, grep, glob over a rooted workspace |
| `niki-policy` | Manual/Auto/YOLO gate, shell allow-list, path boundary |
| `niki-agent` | the turn loop, event callback, approval callback, step budget, cancel |
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
cargo test -j 2 --workspace        # 191 tests, 0 failures
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

## Next: W4 — extensions

`niki-mcp`, `niki-skills`, `niki-hooks`, `niki-commands`. Slash commands are
currently four hard-coded strings in `App::slash`; W4 should move them into
`niki-commands` and have `niki-tui-live` call it, keeping `/quit`, `/help`,
`/model` and `/theme` as the built-ins.

## Known gaps

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
