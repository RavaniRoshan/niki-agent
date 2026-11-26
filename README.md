<div align="center">

# ◐ Niki Agent

*A terminal coding agent that stays quiet, starts fast, and runs on your machine.*

[![Python](https://img.shields.io/badge/python-3.12%2B-blue?style=flat-square)](https://www.python.org)
[![Textual](https://img.shields.io/badge/terminal-Textual%208-5c873a?style=flat-square)](https://textual.textualize.io)
[![Rust](https://img.shields.io/badge/client-rust%202024-b7410e?style=flat-square)](rust/)
[![License: MIT](https://img.shields.io/badge/license-MIT-silver?style=flat-square)](NOTICE)

[Install](#install) · [First five minutes](#first-five-minutes) · [Approval modes](#approval-modes) ·
[Themes](#themes) · [Providers](#providers) · [Rust client](#rust-client)

</div>

<div align="center">
  <img src="intro.gif" width="900" alt="Niki Agent in a terminal">
</div>

Niki Agent is a coding agent you run in your terminal. You say what you want; it
reads your repository, edits files, runs commands, and shows you each step. It
uses the model you already pay for, keeps its sessions on your own disk, and
stops to ask before it changes anything.

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
niki
```

No account and no API key are needed to look around: the first launch asks your
name and leaves you at a working composer. Add a key when you want it to work:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
```

## First five minutes

```bash
niki                             # interactive session in the current directory
niki -n "summarize this repo"    # one answer on stdout, no TUI
niki --help                      # options, rendered as the key map
niki --version                   # the installed version
```

Type a request and press Enter. `/model` picks a provider and model, `/auth`
stores a key, `/theme` changes the palette, and `/help` lists every command.
`Ctrl+Shift+P` opens a searchable command palette and `Ctrl+J` inserts a
newline. The complete keymap is in [`docs/niki/KEYMAP.md`](docs/niki/KEYMAP.md).

Niki is for people who already work in a terminal: it fits the shell you have,
reads the repository you are in, and never takes over the screen.

## Why this one

A generic coding agent gives you a chat box and a lot of motion. Niki is built
around the parts of a long session that actually cost you: attention, startup
time, and the decision of what the agent may do on its own.

| | |
| --- | --- |
| **A quiet interface** | An idle session paints nothing (a test asserts zero idle repaints). The composer is a single rule, not a box, and a long transcript is mounted in a bounded window, so hour two renders like minute one. |
| **A fast start** | `niki --version` is answered before the UI loads, and model setup is deferred until after the first frame, so the composer appears without waiting on a provider. |
| **Your files, your machine** | Sessions, history, skills, and stored credentials live in `~/.niki`. No account, no cloud sync, no telemetry. |
| **Your model** | Anthropic, OpenAI, and Google Gemini work out of the box; eighteen more providers are one extra away, and any OpenAI-compatible endpoint is a config block. |
| **A real approval gate** | Manual, Auto, and YOLO modes, a shell allow-list, and sandbox backends for when you need a hard boundary. |
| **Accessible, not as an afterthought** | Four themes with contrast measured by the test suite, reduced motion, an ASCII glyph set, a screen-reader mode, and snapshots checked at 40, 50, 80, 120, and 160 columns. |
| **A second front end in Rust** | `rust/` is a native client for the same engine, with deterministic replay, so the terminal frame can be pinned in tests. |

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

## Approval modes

Shift+Tab cycles **Manual → Auto → YOLO**; the status bar names the active mode.

| Mode | What runs without asking first |
| --- | --- |
| Manual | Nothing. Shell commands, file writes and edits, deletes, web search, URL fetches, and task delegation all ask. |
| Auto | Routine gated actions run; anything uncertain is reviewed by a classifier model and still asks when the review fails or is unavailable. |
| YOLO | Everything the policy allows runs unsupervised. The first switch asks for a one-time acknowledgement. |

Auto and YOLO are not sandboxes: the agent runs on this machine with your user's
permissions. Use `--yolo`, `--auto-approve`, the `--shell-allow-list`, and the
sandbox backends (`--sandbox`, extras such as `modal` and `daytona`) when you
need a hard boundary. Setting `startup.yolo_switcher = false` in
`~/.niki/config.toml` keeps YOLO out of the Shift+Tab cycle.

## Themes

Four palettes, switchable at runtime with `/theme`:

| Name | For |
| --- | --- |
| `niki` | The default. Cool, flat greys, one accent. |
| `niki-light` | Light terminals. |
| `niki-contrast` | Bright rooms and low-quality panels — tightest pair 6.22:1. |
| `niki-dim` | Dark rooms, where the default reads too bright. |

All four are tested for contrast in truecolor, 256-colour, and 16-colour
terminals: text at 4.5:1 and UI glyphs at 3:1 against every surface they sit on.

```bash
NO_COLOR=1 niki                            # colour off
NIKI_UI_CHARSET_MODE=ascii niki            # ASCII glyph fallback
NIKI_REDUCED_MOTION=1 niki                 # no animation
NIKI_SCREEN_READER=1 niki                  # linear, append-only output
```

## Providers

Anthropic, OpenAI (including Azure OpenAI and ChatGPT subscription login), and
Google Gemini work out of the box. Eighteen more providers ship as optional
extras — Ollama, OpenRouter, NVIDIA, Groq, Together, DeepSeek, Mistral,
Cohere, xAI, Perplexity, Fireworks, Hugging Face, IBM watsonx, Meta, Baseten,
LiteLLM, Amazon Bedrock, and Google Vertex AI.

Anything else with an OpenAI-compatible API is configuration, not code — drop a
block into `~/.niki/config.toml`:

```toml
[models.providers.kimi]
display_name = "Kimi"
class_path = "langchain_openai:ChatOpenAI"   # the OpenAI-compatible client class
base_url = "https://api.moonshot.ai/v1"
api_key_env = "MOONSHOT_API_KEY"
models = ["kimi-k3", "kimi-k2.7-code", "kimi-k2.6"]
```

Full setup, the config keys, gateway examples, and local-model recipes are in
[`docs/niki/PROVIDERS.md`](docs/niki/PROVIDERS.md).

## Rust client

`rust/` is a native front end for the same engine. It speaks the engine's
HTTP/SSE protocol directly, so it needs no Python TUI to draw a transcript.

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

Today the Rust client renders the transcript, the reader mode, and the keymap.
The composer, modals, approvals, and command palette are in the Python client.

## Docs

| | |
| --- | --- |
| [`ARCHITECTURE.md`](docs/niki/ARCHITECTURE.md) | How the agent, the UI, and the tools fit together. |
| [`INDEPENDENCE.md`](docs/niki/INDEPENDENCE.md) | Package, state, and environment guarantees. |
| [`KEYMAP.md`](docs/niki/KEYMAP.md) | Every key binding, generated from the registry. |
| [`PROVIDERS.md`](docs/niki/PROVIDERS.md) | Provider setup and gateway notes. |
| [`site/README.md`](site/README.md) | How the marketing site is built, checked, and deployed. |
| [`SECURITY.md`](SECURITY.md) | How to report a vulnerability, and what is in scope. |

## Security

Niki runs commands and edits files with your permissions. Report vulnerabilities
privately — see [`SECURITY.md`](SECURITY.md). Release archives are covered by
`SHA256SUMS`, and the installers verify it before installing.

## Contributing

```bash
cd libs/code
make check     # lint, types, imports, tests, and repository gates
```

Contributor conventions, repository layout, and release steps are documented in
the repository's `AGENTS.md` files.

## License

MIT. Third-party copyright notices are retained in [`NOTICE`](NOTICE), and every
included directory carries its licence and a provenance note beside it.
