<div align="center">

# niki

**A quiet terminal coding agent, written in Rust.**

One native binary. Eleven providers. Sessions on your disk.
An approval gate in front of anything that changes your machine.

[![Release](https://img.shields.io/badge/release-v2.1.4-8ec7ff?style=flat-square)](https://github.com/RavaniRoshan/niki-agent/releases)
[![Rust](https://img.shields.io/badge/rust-2024-b7410e?style=flat-square)](rust/)
[![License: MIT](https://img.shields.io/badge/license-MIT-silver?style=flat-square)](LICENSE)

[Install](#install) · [Beta](#beta) · [Documentation](https://niki-docs.vercel.app) · [Changelog](CHANGELOG.md)

</div>

<div align="center">
  <img src="intro.gif" width="1000" alt="A Niki Agent session: it reads src/weather.rs, asks to write a retry helper, runs the tests, and reports the result">
</div>

<p align="center"><sub>Recorded from the shipped binary against a loopback provider, so no key or network is involved.</sub></p>

## What it is

Niki is a coding agent that runs in your terminal. You say what you want; it
reads the repository, edits files, runs commands, and shows each step. It uses
the model you already pay for, keeps its sessions on your own disk, and stops to
ask before it changes anything.

It is one self-contained executable. There is no interpreter to install, no
virtual environment to create, and nothing to resolve on first run.

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
niki
```

No account and no API key are needed to look around: the first launch lands on a
working composer. Add a key when you want it to work:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
```

## Install

**Linux, WSL, and macOS**

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
```

**Windows**

```powershell
irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.ps1 | iex
```

The installer picks the archive for your platform, verifies it against the
release's `SHA256SUMS`, and places one executable under `~/.local/niki`
(`%LOCALAPPDATA%\niki` on Windows). It does not touch your shell profile unless
you pass `NIKI_SHELL=1`. To remove Niki, delete `~/.local/niki` and `~/.niki`.

Linux, WSL, and macOS are exercised by the release workflow. The Windows archive
is built by CI but has not been executed on Windows; the
[beta page](https://niki-docs.vercel.app/beta) tracks platform support.

Requires a 64-bit Linux, macOS (Apple silicon or Intel), or Windows machine.
Building from source needs Rust 1.85 or newer:

```bash
git clone https://github.com/RavaniRoshan/niki-agent.git
cd niki-agent/rust
cargo build --release -p niki --bin niki
```

## Why this one

A generic agent gives you a chat box and a lot of motion. Niki is built around
what a long session actually costs you: attention, startup time, and the
decision of what the agent may do on its own.

| | |
| --- | --- |
| **One binary** | Turn loop, providers, tools, policy, sessions, MCP, and UI are one native executable. |
| **Quiet by default** | An idle session draws nothing. The composer is a single rule, not a box. |
| **Starts instantly** | Native process launch; no interpreter to boot, no import graph to warm. |
| **Your files stay yours** | Sessions, skills, and credentials live in `~/.niki`. No account, no cloud sync. |
| **Your model** | Anthropic, OpenAI, Gemini, Groq, OpenRouter, Together, DeepSeek, NVIDIA, Mistral, xAI, and Ollama, plus any OpenAI-compatible endpoint. |
| **A real approval gate** | Manual, Auto, and YOLO modes, a shell allow-list, a workspace boundary, and a trust gate for project hooks and skills. |
| **Delegation with a budget** | The `task` tool gives one child agent a fresh history, a fixed step budget, and no way to spawn further children. |
| **Scriptable when you need it** | `niki -n` answers once; `--output-format jsonl` streams a turn as events for your own tooling. |

## First five minutes

```bash
niki                             # interactive session in the current directory
niki -n "summarize this repo"    # one answer on stdout, no full-screen UI
niki --help                      # every flag
```

Type a request and press Enter. `Shift+Tab` cycles approval modes, `/model`
picks a provider and model, `/theme` changes the palette, `/resume` restores a
saved session, `/threads` browses them, `/context` shows what the conversation
costs, `/diff` renders the working tree, and `/help` lists every command.

`AGENTS.md` and `.niki/AGENTS.md` are read from the repository root down to the
working directory, under a 32 KiB budget that truncates by line and says so.
`/init` writes a starter file only when one is missing.

Project hooks, skills, and instruction files run as you, so they load only after
`niki trust` records the workspace. `niki untrust` withdraws the grant and
`niki trust --list` shows what you allowed.

## The approval gate

`Shift+Tab` cycles **Manual → Auto → YOLO**; the status bar names the active
mode.

| Mode | What runs without asking first |
| --- | --- |
| Manual | Every gated action asks, except shell commands on the allow-list. |
| Auto | Allow-listed shell commands and writes inside the workspace run; everything else asks. |
| YOLO | Everything the policy allows runs unsupervised. |

None of the modes is a sandbox: Niki runs with your user's permissions. The
workspace path boundary keeps file tools inside the project unless the
configuration widens it. A headless run has nobody to ask, so a gated action is
reported back to the model as a refusal; pass `--mode auto` or `--mode yolo` when
that is what you want.

## Providers

Eleven providers are built in. Name the model and set the provider's key
variable:

```toml
[models]
default = "groq:llama-3.3-70b-versatile"
```

Anything else with an OpenAI-compatible API is configuration, not code:

```toml
[models]
default = "moonshot:kimi-k3"

[providers.moonshot]
display_name = "Moonshot"
base_url = "https://api.moonshot.ai/v1"
api_key_env = "MOONSHOT_API_KEY"
models = ["kimi-k3", "kimi-k2.7-code"]
```

MCP servers live in the same file, with `${VAR}` expanded from the environment:

```toml
[mcp_servers.docs]
command = "npx"
args = ["-y", "@example/docs-server"]
timeout = 30
```

Setup details, gateway examples, and the full config key list are in
[`docs/niki/PROVIDERS.md`](docs/niki/PROVIDERS.md).

## Beta

Niki is usable today and still moving. Beta builds ship as GitHub
pre-releases: they pass the same Rust test gate as a stable release but may
change behaviour between builds.

When one is available, pin it by its version (no leading `v`):

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh \
  | NIKI_VERSION=2.2.0-beta.1 bash
```

Known gaps, the support matrix, and how to report a problem are on the
[beta page](https://niki-docs.vercel.app/beta). Feedback belongs in the
[beta feedback issue form](https://github.com/RavaniRoshan/niki-agent/issues/new?template=beta-feedback.yml);
the [roadmap issue](https://github.com/RavaniRoshan/niki-agent/issues/114) lists
what is shipped and what is next. Every release is written up in
[`CHANGELOG.md`](CHANGELOG.md).

## Docs

| | |
| --- | --- |
| [Documentation site](https://niki-docs.vercel.app) | Install, first turn, providers, keymap, permissions, security. |
| [`docs/niki/KEYMAP.md`](docs/niki/KEYMAP.md) | Every key binding. |
| [`docs/niki/ARCHITECTURE.md`](docs/niki/ARCHITECTURE.md) | How the agent, the UI, and the tools fit together. |
| [`docs/niki/INDEPENDENCE.md`](docs/niki/INDEPENDENCE.md) | Package, state, and environment guarantees. |
| [`SECURITY.md`](SECURITY.md) | How to report a vulnerability, and what is in scope. |

## Development

```bash
cd rust
cargo fmt --all -- --check
cargo clippy -j 2 --workspace --all-targets -- -D warnings
cargo test -j 2 --workspace
```

The workspace is split by responsibility: `niki-agent` owns the turn loop,
`niki-provider` the wire clients, `niki-tools` the filesystem and shell tools,
`niki-policy` the approval decisions, `niki-session` the SQLite history,
`niki-mcp` the MCP clients, and `niki-tui*` the full-screen app. `rust/MAP.md`
maps the crates. `libs/code` holds the earlier Python implementation; it is
deprecated, reference-only, and not what a user installs.

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for setup, the checks CI runs, and
commit conventions. Release steps are in [`.github/RELEASING.md`](.github/RELEASING.md).

## Security

Niki runs commands and edits files with your permissions. Report
vulnerabilities privately — see [`SECURITY.md`](SECURITY.md). Release archives
are covered by `SHA256SUMS`, and both installers verify it before installing.

## License

MIT. Third-party copyright notices are retained in [`NOTICE`](NOTICE), and every
included directory carries its licence and a provenance note beside it.
