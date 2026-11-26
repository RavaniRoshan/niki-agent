# Handoff — after W1 and W2

State: W0, W1 and W2 are green on `shiva/rust/w1`. The next session starts W3.

## What is on the branch

| Crate | What it answers from Rust |
| --- | --- |
| `niki-config` | `config.toml` parsing: provider blocks, `[models].default`, `[ui].theme` |
| `niki-provider` | one streaming turn against OpenAI-compatible or Anthropic endpoints |
| `niki-tools` | read/write/edit/list, shell, grep, glob over a rooted workspace |
| `niki-policy` | Manual/Auto/YOLO gate, shell allow-list, path boundary |
| `niki-agent` | the turn loop, event callback, step budget, cancel handle |
| `niki-workspace` | root detection, ignore rules, file index |
| `niki-session` | SQLite transcript store, resume, checkpoints |

`niki-rs -n "prompt"` is the headless entry point. `niki` still replays a
protocol fixture and streams from the Python engine; it is unchanged.

## Gates

```
cd rust
cargo fmt --all
cargo clippy -j 2 --workspace --all-targets -- -D warnings
cargo test -j 2 --workspace        # 96 tests, 0 failures
```

`crates/niki/tests/niki_rs.rs` is the W1 exit gate: it binds a mock OpenAI SSE
server on loopback, runs the binary as a child process, and asserts the file the
model asked for exists and that the tool result travelled back in the second
request body.

## Decisions a new session must know

- The agent assembles tool calls from `StreamEvent::ToolCallDelta` itself, using
  `niki_provider::openai::ToolCallAssembler`. The adapters emit fragments; nobody
  else may assume whole calls.
- A tool failure and a policy refusal are **tool results**, not `AgentError`s, so
  the model can react. Only provider failures abort a run.
- `Decision::Ask` is refused in a headless run, because there is nobody to ask.
  The refusal message tells the model what would unblock it.
- The binary is `niki-rs` until W6 (RULES.md 2).

## Next: W3 — interactive TUI

1. `niki-tui-live`: ratatui full screen — composer, transcript viewport, status
   bar. Drive it from `AgentEvent`, run the loop on a worker and feed the UI.
2. `niki-tui-modals`: approval prompt wiring `Decision::Ask` to a real answer
   instead of a refusal; model picker reading `Config::providers`; help from
   `BINDINGS`.
3. `niki-tui-themes`: four palettes, a contrast test, ASCII glyphs, reduced
   motion.

W3 turns `Decision::Ask` into a real question: the gate must become an async
callback the UI answers, replacing the headless refusal. That is the one
interface change the loop needs.

## Known gaps

- No streaming cancel inside a provider request beyond the per-event check.
- `niki-engine-client` and `niki-proto` still describe themselves in terms of the
  runtime the Python engine speaks; their crate descriptions need a product-level
  rewrite when the engine is retired (W6).
- Token counting is provider-reported only (ADR-005 open question 1).
