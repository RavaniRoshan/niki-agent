---
layout: home
title: Niki Agent
titleTemplate: false

hero:
  name: Niki Agent
  text: A Rust terminal coding agent in one binary
  tagline: You say what you want; it reads your repository, edits files, runs commands, and shows you each step. It uses the model you already pay for, keeps its sessions on your own disk, and asks before it runs a shell command.
  actions:
    - theme: brand
      text: Install
      link: /getting-started/install
    - theme: alt
      text: Your first turn
      link: /getting-started/first-turn
    - theme: alt
      text: Providers
      link: /providers/

features:
  - title: One binary, no runtime
    details: The turn loop, tool registry, approval policy, provider adapters, MCP client, hooks, skills, session store and terminal UI are Rust crates linked into one niki executable. There is no interpreter or package manager beside it.
  - title: Your model
    details: Eleven providers have a built-in endpoint and key variable, including Anthropic, OpenAI, Google Gemini and Ollama. Any OpenAI-compatible gateway is a `[providers.<name>]` block in `config.toml`.
  - title: Your files, your machine
    details: Configuration, skills, hooks and the SQLite session store live under ~/.niki. NIKI_HOME moves the whole tree. No account, no cloud sync, and no telemetry.
  - title: A real approval gate
    details: Manual, Auto and YOLO modes, a shell allow-list, and a workspace path boundary that refuses a path outside the root. Mutating MCP tools are gated too.
  - title: Headless or editor-driven
    details: niki -n runs one prompt, writes the answer to stdout and tool activity to stderr, and exits 0, 1, 2, 124 or 130. niki --acp serves the Agent Client Protocol on stdin/stdout so an editor can drive the same agent.
  - title: Accessible by construction
    details: Four palettes with WCAG contrast measured in the test suite, an ASCII glyph mode, and a reduced-motion setting.
---

## Install

Linux x86_64 has a prebuilt release archive. Verify it against `SHA256SUMS`,
then run the installer inside the archive or place the binary yourself:

```bash
VERSION=2.1.1
curl -fLO "https://github.com/RavaniRoshan/niki-agent/releases/download/v${VERSION}/niki-${VERSION}-linux-x86_64.tar.gz"
curl -fLO "https://github.com/RavaniRoshan/niki-agent/releases/download/v${VERSION}/SHA256SUMS"
grep "niki-${VERSION}-linux-x86_64.tar.gz" SHA256SUMS | sha256sum -c -
tar -xzf "niki-${VERSION}-linux-x86_64.tar.gz"
bash "niki-${VERSION}-linux-x86_64/install.sh"
```

macOS publishes `niki-<version>-macos-aarch64.tar.gz` and
`niki-<version>-macos-x86_64.tar.gz`; the installer chooses by `uname -m`.
Windows has a `.zip` archive that has not been exercised on Windows, so treat it
as untested.

Then name a model in `~/.niki/config.toml` and export its key:

```toml
[models]
default = "anthropic:my-model"
```

```bash
export ANTHROPIC_API_KEY=sk-ant-...
niki
```

[Install in detail](/getting-started/install) ·
[Your first turn](/getting-started/first-turn) ·
[Every key binding](/keyboard) · [Provider setup](/providers/)

## Honest gaps

There is no sandbox and no classifier in this build: Auto asks like Manual, and
built-in file writes run without an approval prompt. Run Niki in a container or
under a dedicated user account when you need a hard boundary, and read
[Permissions](/permissions) and [Security](/security) for the exact gate.

## Documentation

| | |
| --- | --- |
| [Install](/getting-started/install) | Linux, macOS and Windows release archives, plus a source build. |
| [Your first turn](/getting-started/first-turn) | Flags, exit codes, keys, themes, and headless mode. |
| [Providers](/providers/) | Eleven built-ins and any OpenAI-compatible endpoint. |
| [Permissions](/permissions) | Manual, Auto, YOLO, the allow-list, MCP approval, and hooks. |
| [Keyboard reference](/keyboard) | Every binding the app acts on, and the five that are not wired yet. |
| [Extensions](/extensions/) | Skills, hooks, and MCP servers. |
| [Architecture](/architecture) | The crates and the turn loop. |
| [Independence](/independence) | Binary, state, and environment guarantees. |
| [Security](/security) | Trust boundaries, known gaps, and hardening. |
| [Troubleshooting](/troubleshooting) | What each failure message means. |

The marketing site is at [niki-site.vercel.app](https://niki-site.vercel.app),
and the repository is
[github.com/RavaniRoshan/niki-agent](https://github.com/RavaniRoshan/niki-agent).
