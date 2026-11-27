<div align="center">

# ◐ Niki Agent

*A terminal coding agent that stays quiet, starts fast, and runs on your machine.*

<sub>Current release: <a href="https://github.com/RavaniRoshan/niki-agent/releases/tag/v2.1.1">v2.1.1</a> · <a href="https://niki-docs.vercel.app">Documentation</a> · <a href="https://niki-site.vercel.app">Website</a></sub>

[![Rust](https://img.shields.io/badge/rust-2024-b7410e?style=flat-square)](rust/)
[![Release](https://img.shields.io/badge/release-v2.1.1-blue?style=flat-square)](https://github.com/RavaniRoshan/niki-agent/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-silver?style=flat-square)](LICENSE)

[Install](#install) · [First five minutes](#first-five-minutes) · [Approval modes](#approval-modes) ·
[Themes](#themes) · [Providers](#providers) · [Development](#development)

</div>

<div align="center">
  <img src="intro.gif" width="900" alt="Niki Agent in a terminal">
</div>

Niki Agent is a coding agent you run in your terminal. You say what you want; it
reads your repository, edits files, runs commands, and shows you each step. It
uses the model you already pay for, keeps its sessions on your own disk, and
stops to ask before it changes anything.

The product is a single self-contained Rust binary. There is no Python
interpreter, no virtual environment, and no package resolution on first run.

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
niki
```

No account and no API key are needed to look around: the first launch leaves you
at a working composer. Add a key when you want it to work:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
```

## First five minutes

```bash
niki                             # interactive session in the current directory
niki -n "summarize this repo"    # one answer on stdout, no TUI
niki --help                      # flags
niki --version                   # the installed version
```

Type a request and press Enter. `Shift+Tab` cycles approval modes, `/model`
picks a provider and model, `/theme` changes the palette, `/skills` lists
project and user skills, `/resume` restores a saved session, `/threads` browses
them by first prompt, `/context` shows what the conversation costs, `/diff`
renders the working tree, and `/help` lists every command. `/rename` names the
current session and `/export` writes it out as Markdown. The complete keymap is
in [`docs/niki/KEYMAP.md`](docs/niki/KEYMAP.md).

The composer keeps up with the turn: Enter while one is running queues the next
message, `Alt+Enter` or `Ctrl+Enter` steers the running turn, and `Esc Esc` on
an empty composer takes the last turn back out of the view and the history. A
line beginning with `!` runs locally through the same allow-list and costs no
tokens. `Ctrl+T` opens the whole conversation in a scrollable overlay when the
viewport is not enough.

Instructions are files, not settings: `AGENTS.md` and `.niki/AGENTS.md` are read
from the repository root down to the working directory, with your user-level
file first, under a 32 KiB budget that truncates by line and says so. `/init`
writes a starter file only when one is missing.

For scripts, `niki -n` answers once on stdout, `--output-format jsonl` streams a
turn as one event per line (`start`, `text`, `tool_call`, `tool_result`,
`usage`, `end`), and `--mode auto` or `--mode yolo` answers the gate a headless
run cannot.

Niki is for people who already work in a terminal: it fits the shell you have,
reads the repository you are in, and never takes over the screen.

## Why this one

A generic coding agent gives you a chat box and a lot of motion. Niki is built
around the parts of a long session that actually cost you: attention, startup
time, and the decision of what the agent may do on its own.

| | |
| --- | --- |
| **One binary** | The turn loop, providers, tools, policy, sessions, MCP client, and UI are one native executable. Nothing imports a language runtime. |
| **A quiet interface** | An idle session draws nothing: the loop wakes on keys, worker events and a moving spinner, never on a timer that repaints the world. The composer is a single rule, not a box. |
| **A fast start** | Startup is a native process launch — no interpreter to boot and no import graph to warm. |
| **Your files, your machine** | Sessions, history, skills, and stored credentials live in `~/.niki`. No account and no cloud sync. |
| **Your model** | Anthropic, OpenAI, Google Gemini, Groq, OpenRouter, Together, DeepSeek, NVIDIA, Mistral, xAI, and local Ollama are built in; any OpenAI-compatible endpoint is a config block. |
| **Delegation with a budget** | The `task` tool hands one self-contained job to a child agent with an empty history, no way to spawn further children, and a fixed step budget. You get the answer, not a second transcript. |
| **It knows what it costs** | Token usage accumulates across turns; `/context` reports the conversation's size and the session total, and `/compact` folds older turns through the model instead of dropping them silently. |
| **A real approval gate** | Manual, Auto, and YOLO modes, a shell allow-list, a workspace path boundary, and built-in writes gated the same way. |
| **The working tree in place** | `/diff` shows staged, unstaged, and untracked changes in the transcript, rendered by the same diff renderer the agent's edits use. |
| **The whole conversation on demand** | `Ctrl+T` opens every prompt, answer, tool call and result in a scrollable overlay; `Esc` returns to the composer. |
| **Sessions you can name and keep** | `/threads` finds a session by its first prompt, `/rename` gives it a title, `/export` writes the whole thing out as Markdown. |
| **Accessible, not as an afterthought** | Four themes with contrast measured by the test suite, reduced motion, and glyph fallbacks for terminals that need them. |
| **Extensible without a plugin runtime** | MCP servers over stdio or HTTP, project and user skills as plain files, lifecycle hooks, and an optional `[notify]` program that runs once when a turn ends. |

## Install

### From a release

Linux, WSL, and Windows:

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
```

```powershell
irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.ps1 | iex
```

The installer downloads the platform archive, verifies it against the release's
`SHA256SUMS`, and installs one executable under `~/.local/niki`
(`%LOCALAPPDATA%\niki` on Windows). It never edits your shell profile unless you
ask with `NIKI_SHELL=1`. To uninstall, remove `~/.local/niki` (the application)
and `~/.niki` (your configuration and sessions).

macOS has no prebuilt asset yet; build it from source (below).

### From source

Requires a Rust toolchain (1.85+). No Python is needed.

```bash
git clone https://github.com/RavaniRoshan/niki-agent.git
cd niki-agent/rust
cargo build --release -p niki --bin niki
install -m 0755 target/release/niki ~/.local/niki/bin/niki
```

## Approval modes

Shift+Tab cycles **Manual → Auto → YOLO**; the status bar names the active mode.

| Mode | What runs without asking first |
| --- | --- |
| Manual | Nothing. Shell commands, file writes and edits, and deletes all ask. |
| Auto | Shell commands on the allow-list and paths inside the workspace run; everything else asks. |
| YOLO | Everything the policy allows runs unsupervised. |

None of the modes is a sandbox: the agent runs on this machine with your user's
permissions. The workspace path boundary keeps file tools inside the project
unless the configuration widens it.

A headless run (`-n`) has nobody to ask, so a gated action is reported back to
the model as a refusal. Pass `--mode auto` to let it write inside the workspace,
or `--mode yolo` to let it run unsupervised.

## Themes

Four palettes, switchable at runtime with `/theme`:

| Name | For |
| --- | --- |
| `niki` | The default. Cool, flat greys, one accent. |
| `high-contrast` | Bright rooms and low-quality panels. |
| `colorblind` | Colour-blind-safe hues. |
| `mono` | No colour; shape and weight only. |

All four are tested for contrast in truecolor, 256-colour, and 16-colour
terminals: text at 4.5:1 and UI glyphs at 3:1 against every surface they sit on.

```bash
NIKI_REDUCED_MOTION=1 niki     # no animation
```

Glyphs fall back to ASCII when the terminal cannot draw them.

## Providers

Anthropic, OpenAI, Google Gemini (OpenAI-compatible surface), Groq, OpenRouter,
Together, DeepSeek, NVIDIA, Mistral, xAI, and Ollama are built in: name the
model and set the provider's key variable.

Anything else with an OpenAI-compatible API is configuration, not code — drop a
block into `~/.niki/config.toml`:

```toml
[models]
default = "kimi:kimi-k3"

[models.providers.kimi]
display_name = "Kimi"
base_url = "https://api.moonshot.ai/v1"
api_key_env = "MOONSHOT_API_KEY"
models = ["kimi-k3", "kimi-k2.7-code", "kimi-k2.6"]
```

`protocol` defaults to OpenAI Chat Completions for unknown providers and
`anthropic` for Anthropic endpoints. MCP servers are configured beside it:

```toml
[mcp_servers.docs]
command = "npx"
args = ["-y", "@example/docs-server"]
```

Full setup, the config keys, and gateway examples are in
[`docs/niki/PROVIDERS.md`](docs/niki/PROVIDERS.md).

## Development

The shipped client is the Rust workspace under `rust/`:

```bash
cd rust
cargo fmt --all -- --check
cargo clippy -j 4 --workspace --all-targets -- -D warnings
cargo test -j 4 --workspace
```

| Crate | What it owns |
| --- | --- |
| `niki-config` | `~/.niki/config.toml`, provider blocks, built-in providers |
| `niki-provider` | OpenAI and Anthropic wire clients, SSE streaming, tool calls |
| `niki-agent` | the turn loop: model → tools → results → model, usage accounting, compaction |
| `niki-tools` | file read/write/edit/list, shell, grep, glob |
| `niki-subagents` | the `task` tool: a child agent with its own history and a step budget |
| `niki-commands` | the slash-command catalogue, aliases, and the not-yet list |
| `niki-policy` | Manual/Auto/YOLO, shell allow-list, path boundary |
| `niki-workspace` / `niki-session` | project root, ignore rules, SQLite sessions |
| `niki-skills` / `niki-hooks` | project and user skills, lifecycle hooks |
| `niki-mcp` | stdio and HTTP MCP clients |
| `niki-render` | Markdown and diff rendering |
| `niki-tui*` | the full-screen terminal app, modals, and themes |
| `niki` | the `niki` binary |

`libs/code` holds the Python reference implementation. It is retained as the
behavioural reference and fixture source; it is no longer installed or shipped
as the product.

## Docs

| | |
| --- | --- |
| [Documentation site](https://niki-docs.vercel.app) | The same guides as a site: install, first turn, providers, keymap, security. |
| [`ARCHITECTURE.md`](docs/niki/ARCHITECTURE.md) | How the agent, the UI, and the tools fit together. |
| [`INDEPENDENCE.md`](docs/niki/INDEPENDENCE.md) | Package, state, and environment guarantees. |
| [`KEYMAP.md`](docs/niki/KEYMAP.md) | Every key binding. |
| [`PROVIDERS.md`](docs/niki/PROVIDERS.md) | Provider setup and gateway notes. |
| [`SECURITY.md`](SECURITY.md) | How to report a vulnerability, and what is in scope. |

## Security

Niki runs commands and edits files with your permissions. Report vulnerabilities
privately — see [`SECURITY.md`](SECURITY.md). Release archives are covered by
`SHA256SUMS`, and the installers verify it before installing.

## Contributing

```bash
cd rust && cargo test -j 4 --workspace   # the shipped client
cd libs/code && make check               # the Python reference implementation
```

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for setup, the checks CI runs, and
commit conventions. Release steps are in [`.github/RELEASING.md`](.github/RELEASING.md).

## License

MIT. Third-party copyright notices are retained in [`NOTICE`](NOTICE), and every
included directory carries its licence and a provenance note beside it.
