# Niki Agent

A calm, fast, keyboard-native terminal coding agent. A presentation-layer fork
of [Deep Agents](https://github.com/langchain-ai/deepagents) — the agent, tools,
permissions, and SDK are upstream's and unchanged.

## Install

**Linux / macOS**

```bash
curl -fsSL https://github.com/RavaniRoshan/niki-agent/releases/latest/download/install.sh | bash
```

**Windows (PowerShell)**

```powershell
irm https://github.com/RavaniRoshan/niki-agent/releases/latest/download/install.ps1 | iex
```

Both create an isolated environment under `~/.local/niki`
(`%LOCALAPPDATA%\niki` on Windows) and never touch your shell profile unless you
ask with `NIKI_SHELL=1`. Requires Python 3.12+.

Then set a provider key and run it:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
niki
```

## What this release contains

- `niki` as a command beside an untouched `dcode`
- Four contrast-verified theme variants, switchable with `/theme`
- A ping-ponging activity indicator with `NIKI_REDUCED_MOTION` support
- A bounded mount window: render cost grew 2.09× with transcript depth before,
  1.21× after
- Idle repaints over 5 seconds: 10 before, 0 after

## Honest status

**26 WORKS · 33 PARTIAL · 11 MISSING** of 76 checklist rows, recorded in
[`docs/niki/CHECKLIST.md`](https://github.com/RavaniRoshan/niki-agent/blob/main/docs/niki/CHECKLIST.md).

The open rows are mostly visual work that needs a real terminal to judge — the
synthetic test driver cannot decide them, because a synthetic assistant message
lays out at height 0 and paints nothing. They are listed rather than smoothed
over.

## Artifacts

| File | Platform |
| --- | --- |
| `niki-<version>-linux.tar.gz` | Linux x86-64 |
| `niki-<version>-macos-arm64.tar.gz` | macOS Apple Silicon |
| `niki-<version>-macos-x64.tar.gz` | macOS Intel |
| `niki-<version>-windows.zip` | Windows x86-64 |

Each contains the pinned wheel, the installer, `NOTICE`, and `README.md`.

## Attribution

Built on Deep Agents by LangChain, MIT licensed. Upstream copyright and notices
are retained verbatim — see [`NOTICE`](https://github.com/RavaniRoshan/niki-agent/blob/main/NOTICE).

The distribution name upstream uses, `deepagents-code`, is deliberately
unchanged so that the package remains installable and attributable.