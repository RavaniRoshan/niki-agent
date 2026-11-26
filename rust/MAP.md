# Niki Agent — whole-app port map

Goal: a standalone Rust program that is the product, with the Python package kept
only as reference. The port outlives one session, so this file carries it.

Read order for a new session: this file, then `DECISIONS.md`, `GLOSSARY.md`,
`RULES.md`, then `HANDOFF.md` when it exists.

## State of the port

| | |
| --- | --- |
| Source | `libs/code` — 377 files, ~250k lines, 41 analyzable units |
| Rust today | `rust/crates/` — 7,941 lines across 12 crates |
| Fallback rate | 100% (the binary still runs the Python engine for a live turn) |
| Cutover | not started; Python stays the product until W6 |

The analysis (`rust-anything analyze libs/code`) reports 9 ready, 28 caution and
4 blocked units. The central fact of the port holds: it is a rewrite against the
same external contracts, not a function-by-function translation. Recorded app
cases are the equivalence proof.

W0–W2 are green. The agent has its own model client, tool registry, approval
policy, workspace and session store; the interactive TUI is W3.

## Waves

A unit is `open`, `claimed by <session> <date>`, or `done <gate/replay>`.

### W0 — protocol and shell (done, pre-existing)

| Unit | State |
| --- | --- |
| `niki-proto` — wire events, SSE parsing, fixtures | done |
| `niki-engine-client` — thread create, stream, cancel | done |
| `niki-render` — transcript model, plain-text frame | done |
| `niki-tui` — keymap, reader mode, view state | done |

### W1 — core engine (done)

| Unit | Scope | State |
| --- | --- | --- |
| `niki-config` | `~/.niki/config.toml`, `NIKI_*`, provider blocks, defaults | done — 10 tests |
| `niki-provider` | OpenAI-compatible Chat Completions and Anthropic Messages, SSE streaming, tool-call assembly, retries, usage | done — 6 fixture tests |
| `niki-agent` | turn loop: model → tool calls → results → model, cancel, max steps | done — 7 tests |
| `niki-tools` | fs read/write/edit/list, shell exec, grep, glob, todo write | done — 14 tests |
| `niki-policy` | Manual/Auto/YOLO, shell allow-list, path boundary | done — 13 tests |

W1 ends when `niki-rs -n "prompt"` runs a real turn against a mock provider,
writes a file, and exits with the same transcript shape as the Python headless
mode. That gate is `crates/niki/tests/niki_rs.rs`: a mock OpenAI SSE server
drives `write_file`, the result goes back over the wire as a `tool` message, and
the answer reaches stdout.

### W2 — workspace and sessions (done)

| Unit | Scope | State |
| --- | --- | --- |
| `niki-workspace` | root detection, ignore rules, file index for grep/glob | done — 11 tests |
| `niki-session` | SQLite store, transcript persistence, resume, checkpoints | done — 9 tests |

### W3 — interactive TUI

| Unit | Scope | State |
| --- | --- | --- |
| `niki-tui-live` | ratatui full screen: composer, transcript viewport, status bar | done — 51 tests |
| `niki-tui-modals` | approvals, model picker, theme picker, help | done — 11 tests |
| `niki-tui-themes` | four palettes, contrast test, ASCII glyphs, reduced motion | claimed by agent-24 2026-11-26 |

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

- **Blocked:** the vendored JS interpreter (`niki_agent/_vendor/quickjs`, C++
  runtime, no Rust crate that keeps the PTC API); `niki_agent/sdk/backends/`
  (filesystem and composite backends depend on the vendored graph runtime).
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
