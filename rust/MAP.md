# Niki Agent — whole-app port map

Goal: a standalone Rust program that becomes the product, with the Python package
kept as the reference implementation while the port is built.

Read this file, then `DECISIONS.md`, `GLOSSARY.md`, `RULES.md`, and
`HANDOFF.md` for current status.

## State of the port

| | |
| --- | --- |
| Source | `libs/code` — 377 files, ~250k lines, 41 analyzable units |
| Rust today | `rust/crates/` — 45,063 lines across 21 crates |
| Fallback rate | 0% for a headless or interactive turn; the `niki` engine client is unchanged and is not packaged |
| Cutover | done — v2.0.0 shipped the Rust binary for four platforms; Python is reference-only |

The analysis (`rust-anything analyze libs/code`) reports 9 ready, 28 caution and
4 blocked units. The central fact of the port holds: it is a rewrite against the
same external contracts, not a function-by-function translation. Recorded app
cases are the equivalence proof.

W0–W6 are green: the agent has its own model client, tool registry, approval
policy, workspace, session store and full-screen terminal app; `niki-mcp`
(stdio and HTTP), skills, hooks (`PreToolUse`, `PostToolUse`,
`UserPromptSubmit`), slash commands, the ACP server and the headless surfaces
are all in the binary, and the screen persists every turn.

The waves below are kept as the record of how the port was sequenced.

## Waves

A unit is `open`, `in progress`, or `done <gate/replay>`.

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
| `niki-config` | `~/.niki/config.toml`, `NIKI_*`, provider blocks, defaults | done — 20 tests |
| `niki-provider` | OpenAI-compatible Chat Completions and Anthropic Messages, SSE streaming, tool-call assembly, retries, usage | done — 6 fixture tests |
| `niki-agent` | turn loop: model → tool calls → results → model, cancel, max steps | done — 7 tests |
| `niki-tools` | fs read/write/edit/list, shell exec, grep, glob, todo write | done — 14 tests |
| `niki-policy` | Manual/Auto/YOLO, shell allow-list, path boundary | done — 13 tests |

W1 ends when `niki -n "prompt"` runs a real turn against a mock provider,
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
| `niki-tui-live` | ratatui full screen: composer with bracketed paste, transcript viewport, status bar, session persistence and `/resume` | done — 86 tests |
| `niki-tui-modals` | approvals, model picker, theme picker, help (including the command list) | done — 12 tests |
| `niki-tui-themes` | four palettes, contrast test, ASCII glyphs, reduced motion | done — 7 tests |

### W4 — extensions

| Unit | Scope | State |
| --- | --- | --- |
| `niki-mcp` | stdio and HTTP MCP clients, tool bridging, startup load in every entry point, per-server startup timeout | done — 28 tests |
| `niki-skills` | skill discovery, front matter, injection | done — 31 tests |
| `niki-hooks` | `hooks.json`, pre/post tool and prompt hooks | done — 24 tests |
| `niki-commands` | slash command grammar: `/clear`, `/compact`, `/resume`, `/tools`, `/version`, `/skills` beside the existing four | done — 7 tests |

### W5 — surfaces

| Unit | Scope | State |
| --- | --- | --- |
| `niki-acp` | editor protocol over stdio: initialize, sessions, prompt, cancel, modes, config, streaming updates | done — 27 tests |
| `niki-headless` | `-n`, `--output-format json`, `--continue`/`--resume`, exit 0/1/2/124/130 | done |

### W6 — cutover (done)

Done: the release archive installs `niki` from `rust/` on Linux x86_64, macOS
arm64, macOS x86_64 and Windows x86_64, with `SHA256SUMS` and an installer that
picks by platform; the installed binary runs a turn with no language runtime on
the machine. `libs/code` stays in the tree as the reference implementation, and
its wheel no longer declares a `niki` console script.

What remains against the reference is tracked one row per surface in
[`PARITY.md`](./PARITY.md) rather than as a wave.

## Blocked and kept

- **Blocked:** the vendored JS interpreter (`niki_agent/_vendor/quickjs`, C++
  runtime, no Rust crate that keeps the PTC API); `niki_agent/sdk/backends/`
  (filesystem and composite backends depend on the vendored graph runtime).
  Neither is reachable from the Rust binary, so both stay Python-only.
- **Kept in Python:** the syntax-highlighting grammar set. Token counting is
  provider-reported usage (`Agent::session_usage`), with `context_audit` for the
  next request; there is no local BPE counter and none is planned.

## Out of scope

- Reading the Python session database from Rust (ADR-006).
- Byte-identical help text (ADR-007).
- The deprecated console-script aliases.

## Settled while porting

1. Token counting is provider-reported usage, not a local BPE crate.
2. Skills are files: `<root>/.niki/skills` and `~/.niki/skills`, with `commit`
   and `review` compiled into `niki-skills`. No skills ship as archives.
