# Niki Agent — whole-app port map

Goal: a standalone Rust program that is the product, with the Python package kept
only as reference. The port outlives one session, so this file carries it.

Read order for a new session: this file, then `DECISIONS.md`, `GLOSSARY.md`,
`RULES.md`, then `HANDOFF.md` when it exists.

## State of the port

| | |
| --- | --- |
| Source | `libs/code` — 377 files, ~250k lines, 41 analyzable units |
| Rust today | `rust/crates/` — 1,861 lines: protocol, engine client, render, TUI state |
| Fallback rate | 100% (no unit answers from Rust yet) |
| Cutover | not started; Python stays the product until W6 |

The analysis (`rust-anything analyze libs/code`) reports 9 ready, 28 caution and
4 blocked units, and **no Rust crate for the agent runtime, the UI toolkit, or
the provider integrations**. That is the central fact of this port: it is a
rewrite against the same external contracts, not a function-by-function
translation. Recorded app cases are the equivalence proof.

## Waves

A unit is `open`, `claimed by <session> <date>`, or `done <gate/replay>`.

### W0 — protocol and shell (done, pre-existing)

| Unit | State |
| --- | --- |
| `niki-proto` — wire events, SSE parsing, fixtures | done |
| `niki-engine-client` — thread create, stream, cancel | done |
| `niki-render` — transcript model, plain-text frame | done |
| `niki-tui` — keymap, reader mode, view state | done |

### W1 — core engine (the next wave)

| Unit | Scope | State |
| --- | --- | --- |
| `niki-config` | `~/.niki/config.toml`, `NIKI_*`, provider blocks, defaults | open |
| `niki-provider` | OpenAI-compatible Chat Completions and Anthropic Messages, SSE streaming, tool-call assembly, retries, usage | open |
| `niki-agent` | turn loop: model → tool calls → results → model, cancel, max steps | open |
| `niki-tools` | fs read/write/edit/list, shell exec, grep, glob, todo write | open |
| `niki-policy` | Manual/Auto/YOLO, shell allow-list, path boundary | open |

W1 ends when `niki-rs -n "prompt"` runs a real turn against a mock provider,
writes a file, and exits with the same transcript shape as the Python headless
mode.

### W2 — workspace and sessions

| Unit | Scope | State |
| --- | --- | --- |
| `niki-workspace` | root detection, ignore rules, file index for grep/glob | open |
| `niki-session` | SQLite store, transcript persistence, resume, checkpoints | open |

### W3 — interactive TUI

| Unit | Scope | State |
| --- | --- | --- |
| `niki-tui-live` | ratatui full screen: composer, transcript viewport, status bar | open |
| `niki-tui-modals` | approvals, model picker, theme picker, help | open |
| `niki-tui-themes` | four palettes, contrast test, ASCII glyphs, reduced motion | open |

### W4 — extensions

| Unit | Scope | State |
| --- | --- | --- |
| `niki-mcp` | stdio and HTTP MCP clients, tool bridging | open |
| `niki-skills` | skill discovery, front matter, injection | open |
| `niki-hooks` | pre/post tool hooks | open |
| `niki-commands` | slash commands | open |

### W5 — surfaces

| Unit | Scope | State |
| --- | --- | --- |
| `niki-acp` | editor protocol over stdio | open |
| `niki-headless` | `-n`, JSON output, exit codes | open |

### W6 — parity and cutover

Parity rows, packaging for Linux/WSL (then macOS/Windows), the `niki` binary
renamed, Python retired to reference. Out of scope until W1–W5 are green.

## Blocked and kept

- **Blocked:** `libs/partners/quickjs` (C++ interpreter, no Rust crate that keeps
  the PTC API); `niki_agent/sdk/backends/` (filesystem and composite backends
  depend on the vendored graph runtime).
- **Kept in Python (for now):** the syntax-highlighting grammar set and the
  `tiktoken` token-counting path. Rust replacements are chosen in W1 and recorded
  in `DECISIONS.md`.

## Out of scope

- Reading the Python session database from Rust (ADR-006).
- Byte-identical help text (ADR-007).
- The deprecated console-script aliases.

## Open questions

1. Token counting without `tiktoken`: a Rust BPE crate, or provider-reported
   usage only? Blocks cost display in W1, not the loop.
2. Do skills ship as embedded assets or as files under `~/.niki/skills`? Affects
   packaging in W6.
