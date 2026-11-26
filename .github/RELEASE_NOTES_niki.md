# Niki Agent

**A terminal coding agent that stays quiet, starts fast, and runs on your
machine.**

This is the release that makes Niki Agent a product you can hand to someone: one
command to install, a calm interface that does not fight for attention, and the
whole agent loop in this repository — no account, no telemetry, and no runtime
dependency on another agent package.

## Install

**Linux / macOS**

```bash
curl -fsSL https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.sh | bash
```

**Windows (PowerShell)**

```powershell
irm https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/install/install.ps1 | iex
```

Both create an isolated environment under `~/.local/niki`
(`%LOCALAPPDATA%\niki` on Windows), verify the release checksum, and leave your
shell profile alone unless you pass `NIKI_SHELL=1`. Requires Python 3.12+.

Then:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
niki
```

No key is needed to look around — `niki` boots to a working composer.

## What makes it different

**It is quiet.** Zero idle redraws. The composer is a single rule, not a box.
A long transcript is mounted in a bounded window, so hour three of a session
renders like minute one.

**It starts fast and stays small.** ~190 ms to first paint in process, 255 MB
resident through a long session, no indexer and no background daemon. The Rust
client in `rust/` already answers `--version` in about 1 ms.

**You choose the model.** Nine providers ship — Anthropic, OpenAI, Google,
Ollama, OpenRouter, NVIDIA, Groq, Together, DeepSeek — and any
OpenAI-compatible endpoint is a four-line block in `~/.niki/config.toml`.

**It asks before it acts.** Shift+Tab cycles Manual → Auto → YOLO. Auto sends
anything uncertain to a classifier and still asks when review fails. A shell
allow-list and sandbox backends give you a hard boundary when you need one.

**Accessibility is built in, not bolted on.** Four themes with measured
contrast — text at 4.5:1, UI glyphs at 3:1, zero violations against every
surface. Reduced motion, an ASCII glyph set, a screen-reader mode, and layouts
checked at 40, 50, 80, 120, and 160 columns.

**Your data stays put.** Sessions, memory, and skills live in `~/.niki`. No
account, no sync, no telemetry.

## In this release

- **One command to install**, on Linux, macOS, Windows, and WSL, with the release
  checksum verified before anything is installed.
- **Four contrast-verified themes** — `niki`, `niki-light`, `niki-contrast`, and
  `niki-dim` — switchable at runtime with `/theme`.
- **Approval modes** with a one-time YOLO notice per machine and a
  `[startup].yolo_switcher` setting for people who never want it in the cycle.
- **Managed policy.** Administrators can pin settings in
  `/etc/niki/managed_config.toml`; the user configuration cannot override it.
- **A Rust client in `rust/`** that speaks the engine's HTTP/SSE protocol:
  deterministic replay, a live session, and a plain-text frame that renders in
  about 4 ms.
- **Documentation for users**, not for us: architecture, key bindings, provider
  setup, approval modes, and a security policy.

## Artifacts

| File | Platform |
| --- | --- |
| `niki-<version>-linux.tar.gz` | Linux x86-64 and WSL |
| `niki-<version>-macos.tar.gz` | macOS, Apple Silicon and Intel |
| `niki-<version>-windows.zip` | Windows x86-64 |

`SHA256SUMS` covers all three. Each archive contains the pinned wheel, both
installers, `NOTICE`, and `README.md`. The macOS archive is
architecture-independent because the wheel is pure Python.

To verify by hand:

```bash
sha256sum -c SHA256SUMS --ignore-missing
```

## Upgrading

Re-run the install command. The installer replaces the environment under
`~/.local/niki` and leaves `~/.niki` — your configuration, sessions, and skills —
untouched.

## Uninstalling

```bash
rm -rf ~/.local/niki          # the application
rm -rf ~/.niki                # configuration, sessions, and skills
```

Remove the `PATH` line from your shell profile if you added one.

## Attribution

Niki Agent is an independent implementation. It includes MIT-licensed components;
their copyright notices and licences are retained verbatim in
[`NOTICE`](https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/NOTICE) and
in the `LICENSE` and `PROVENANCE.md` files beside each included directory.
