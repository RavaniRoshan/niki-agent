---
layout: home
title: Niki Agent
titleTemplate: false

hero:
  name: Niki Agent
  text: A terminal coding agent that stays quiet, starts fast, and runs on your machine
  tagline: You say what you want; it reads your repository, edits files, runs commands, and shows you each step. It uses the model you already pay for, keeps its sessions on your own disk, and stops to ask before it changes anything.
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
  - title: A quiet interface
    details: An idle session paints nothing, and a test asserts zero idle repaints. The composer is a single rule rather than a box, and a long transcript is mounted in a bounded window, so hour two of a session renders like minute one.
  - title: A fast start
    details: "niki --version is answered before the UI loads, and model setup is deferred until after the first frame, so the composer appears without waiting on a provider."
  - title: Your files, your machine
    details: Sessions, history, skills, and stored credentials live in ~/.niki. No account, no cloud sync, and no telemetry.
  - title: Your model
    details: Anthropic, OpenAI, and Google Gemini work immediately; eighteen more provider integrations are one /install away; any OpenAI-compatible endpoint is a configuration block.
  - title: A real approval gate
    details: Manual, Auto, and YOLO modes, a shell allow-list, and sandbox backends for when you need a hard boundary.
  - title: Accessible, not as an afterthought
    details: Four themes with contrast measured by the test suite, reduced motion, an ASCII glyph set, a screen-reader mode, and snapshots checked at 40, 50, 80, 120, and 160 columns.
---

## Install

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
`niki` on your `PATH`. It does not edit your shell profile unless you ask with
`NIKI_SHELL=1`.

No account and no API key are needed to look around: the first launch asks your
name and leaves you at a working composer. Add a key when you want it to work:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
niki
```

[Install in detail](/getting-started/install) · [Your first turn](/getting-started/first-turn) ·
[Every key binding](/keyboard) · [Provider setup](/providers/)

## Documentation

| | |
| --- | --- |
| [Install](/getting-started/install) | Linux, macOS, Windows, and from source. |
| [Your first turn](/getting-started/first-turn) | What the first launch asks, and how a turn looks. |
| [Providers](/providers/) | The built-in three, eighteen more integrations one `/install` away, and any OpenAI-compatible endpoint. |
| [Permissions and sandboxing](/permissions) | Manual, Auto, YOLO, the shell allow-list, and sandbox backends. |
| [Keyboard reference](/keyboard) | Every binding, generated from the live registry. |
| [Extensions and hooks](/extensions/) | Skills, plugins, lifecycle hooks, and Python extensions. |
| [Architecture](/architecture) | How the agent, the UI, and the tools fit together. |
| [Independence](/independence) | Package, state, and environment guarantees. |
| [Security](/security) | Trust boundaries, reporting, and hardening. |
| [Troubleshooting](/troubleshooting) | Where the logs are and what each symptom means. |
