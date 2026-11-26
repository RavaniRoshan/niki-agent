<div align="center">

# ◐ Niki Agent

*A terminal coding agent that stays quiet, starts fast, and runs on your machine.*

[![Python](https://img.shields.io/badge/python-3.12%2B-blue?style=flat-square)](https://www.python.org)
[![Textual](https://img.shields.io/badge/terminal-Textual%208-5c873a?style=flat-square)](https://textual.textualize.io)
[![Rust](https://img.shields.io/badge/client-rust%202024-b7410e?style=flat-square)](rust/)
[![License: MIT](https://img.shields.io/badge/license-MIT-silver?style=flat-square)](NOTICE)

[Install](#install) · [Quickstart](#quickstart) · [Approval modes](#approval-modes) ·
[Themes](#themes) · [Providers](#providers) · [Rust client](#rust-client)

</div>

<div align="center">
  <img src="intro.gif" width="900" alt="Niki Agent in a terminal">
</div>

Niki Agent is a terminal coding agent with its whole agent loop in this
repository. It runs on any model that supports tool calling, keeps your sessions
and memory on your machine, and asks before it does something you did not ask
for.

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
niki
```

No API key is needed to try the interface — `niki` boots straight to a working
composer. Set a key when you want it to actually do something:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
```

## Why this one

| | |
| --- | --- |
| **A quiet interface** | Zero idle redraws. The composer is a single rule, not a box. A long transcript is mounted in a bounded window, so a two-hour session renders like a two-minute one. |
| **A fast start** | ~190 ms to first paint in process, against ~1 ms for `niki --version` and ~4 ms for the first Rust replay frame. |
| **Small footprint** | 255 MB resident through a long session — no background indexer, no daemon. |
| **Provider freedom** | Nine providers ship, and any OpenAI-compatible endpoint is a four-line block in `~/.niki/config.toml`. |
| **Local by default** | Sessions, memory, and credentials stay in `~/.niki`. No account, no telemetry. |
| **Accessible, not as an afterthought** | Four themes with measured contrast, reduced motion, an ASCII glyph set, a screen-reader mode, and layouts checked at 40/50/80/120/160 columns. |
| **A real approval gate** | Manual, Auto, and YOLO modes, a shell allow-list, and sandbox backends for when you need a hard boundary. |

## Install

### From a release

Linux and macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.ps1 | iex
```

The installer creates an isolated environment under `~/.local/niki`
(`%LOCALAPPDATA%\niki` on Windows), verifies the release checksum, and puts
`niki` on your `PATH`. It never edits your shell profile unless you ask with
`NIKI_SHELL=1`.

### From source

Requires Python 3.12+ and [uv](https://docs.astral.sh/uv/).

```bash
git clone https://github.com/RavaniRoshan/niki-agent.git
cd niki-agent/libs/code
uv sync --all-extras
uv run niki
```

Install one provider's extra instead of all of them with
`uv sync --extra anthropic`.

## Quickstart

```bash
niki                                  # interactive
niki --help                           # usage and options
niki --version                        # Niki Agent 1.1.1
niki -n "summarize this repo"         # headless, one answer to stdout
```

In the session: type a request and press Enter. `/help` lists commands, `/theme`
switches palette, `Ctrl+K` opens the command palette, and `Ctrl+J` inserts a
newline. Every binding is in [`docs/niki/KEYMAP.md`](docs/niki/KEYMAP.md).

## Approval modes

Shift+Tab cycles **Manual → Auto → YOLO**; the status bar names the active mode.

| Mode | What runs without asking first |
| --- | --- |
| Manual | Nothing gated. Shell commands, file writes, and network tools ask. |
| Auto | Routine gated actions run; anything uncertain is reviewed by a classifier model and still asks when review fails. |
| YOLO | Everything the policy allows runs unsupervised. The first switch shows a one-time notice per machine. |

Auto and YOLO are not sandboxes: the agent runs on this machine with your user's
permissions. Use `--yolo`, `--auto-approve`, the shell allow-list, and the
sandbox backends when you need a hard boundary. `[startup].yolo_switcher` can
keep YOLO out of the cycle.

## Themes

Four palettes, switchable at runtime with `/theme`:

| Name | For |
| --- | --- |
| `niki` | The default. Cool, flat greys, one accent. |
| `niki-light` | Light terminals. |
| `niki-contrast` | Bright rooms and low-quality panels — tightest pair 6.22:1. |
| `niki-dim` | Dark rooms, where the default reads too bright. |

All four measure **zero contrast violations**: text at 4.5:1, UI glyphs at 3:1,
against every surface they sit on. That is a test, not a claim.

```bash
NO_COLOR=1 niki                            # colour off
NIKI_UI_CHARSET_MODE=ascii niki            # ASCII glyph fallback
NIKI_REDUCED_MOTION=1 niki                 # no animation
```

## Providers

Nine ship already — Anthropic, OpenAI, Google, Ollama, OpenRouter, NVIDIA,
Groq, Together, and DeepSeek. Anything else with an OpenAI-compatible API is
configuration, not code:

```toml
# ~/.niki/config.toml
[providers.kimi]
display_name = "Kimi"
class_path = "langchain_openai:ChatOpenAI"
base_url = "https://api.moonshot.ai/v1"
api_key_env = "MOONSHOT_API_KEY"
models = ["kimi-k3", "kimi-k2.7-code", "kimi-k2.6"]
```

Full setup, gateway notes, and local-model recipes are in
[`docs/niki/PROVIDERS.md`](docs/niki/PROVIDERS.md).

## Design independence

Niki Agent is its own product, not a skin over someone else's runtime:

- **Its own package.** `niki_agent`, published as `niki-agent`.
- **Its own agent loop.** The agent runtime and the editor-protocol integration
  live in this repository. Nothing imports an agent framework at runtime.
- **Its own state.** `~/.niki`, with a documented on-disk layout.
- **Its own environment namespace.** `NIKI_*`, with the same meaning on every
  platform.
- **Its own release pipeline.** Wheels built here, three platform archives,
  checksums published with every release.

The MIT notices for the components that are included are kept in
[`NOTICE`](NOTICE), with a licence and a provenance note beside each.

## Docs

| | |
| --- | --- |
| [`ARCHITECTURE.md`](docs/niki/ARCHITECTURE.md) | How the agent, the UI, and the tools fit together. |
| [`INDEPENDENCE.md`](docs/niki/INDEPENDENCE.md) | Package, state, and environment guarantees. |
| [`KEYMAP.md`](docs/niki/KEYMAP.md) | Every key binding, generated from the registry. |
| [`PROVIDERS.md`](docs/niki/PROVIDERS.md) | Provider setup and gateway notes. |
| [`SECURITY.md`](SECURITY.md) | How to report a vulnerability, and what is in scope. |

## Rust client

`rust/` is a second front end for the same engine, written in Rust. It speaks
the engine's HTTP/SSE protocol directly, so the Python client stays the product
while the Rust binary grows into the fast path.

```bash
cd rust
cargo run -q -p niki -- --version
cargo run -q -p niki -- replay crates/niki-proto/fixtures/hello.json \
  --width 80 --height 24
cargo run -q -p niki -- session --engine http://127.0.0.1:2024 --prompt "Say hello"
```

| Crate | What it owns |
| --- | --- |
| `niki-proto` | wire event names, incremental SSE parsing, committed fixtures |
| `niki-engine-client` | thread creation, streaming runs, cancel, state |
| `niki-render` | transcript model and the deterministic plain-text frame |
| `niki-tui` | keymap, reader mode, and view state |
| `niki` | the binary: `replay`, `session`, `keys` |

Replay is offline and deterministic, which is what lets frames be pinned in
tests. `rust/tools/m2_session.sh` drives a live session inside a PTY and asserts
its final transcript equals the recorded frame.

The current Rust frame renders the transcript region only: the composer, modals,
approvals, and the palette are Python-only until the full-screen shell lands.
The port plan is in [`rust/MAP.md`](rust/MAP.md).

## Tests

```bash
cd libs/code
uv run pytest tests/unit_tests/niki/ -q    # no network, no API key, no terminal
```

## Security

Please report vulnerabilities privately — see [`SECURITY.md`](SECURITY.md).
Release archives are covered by `SHA256SUMS`, and the installers verify it
before installing.

## License

MIT. Third-party copyright notices are retained in [`NOTICE`](NOTICE), and every
included directory carries its licence and a provenance note beside it.
