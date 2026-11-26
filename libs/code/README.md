# ◐ Niki Agent

[![PyPI - Version](https://img.shields.io/badge/pypi-niki--agent-blue)](https://pypi.org/project/niki-agent/)
[![License: MIT](https://img.shields.io/badge/license-MIT-silver)](../../NOTICE)

Niki is a terminal coding agent with an interactive UI, persistent memory,
skills, subagents, and approval controls. It runs on any model that supports
tool calling.

```bash
curl -LsSf https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/libs/code/scripts/install.sh | bash
niki
```

Or from a checkout:

```bash
git clone https://github.com/RavaniRoshan/niki-agent.git
cd niki-agent/libs/code
uv sync --all-extras
uv run niki
```

## What you get

- **Interactive TUI** — streaming responses, tool cards, diffs, themes, and a
  keybinding-driven composer.
- **Headless mode** — `niki -n "..."` for scripts and CI.
- **Sessions** — resume a thread, pick from recent sessions, search transcripts.
- **Memory** — `NIKI.md` / `AGENTS.md` instructions and per-agent profiles.
- **Skills and plugins** — reusable slash commands and Python extensions.
- **Sandboxes** — run tool calls in Daytona, Modal, Runloop, Vercel, or
  LangSmith sandboxes instead of on your machine.
- **Approval modes** — from approve-every-write to a reviewed auto mode.

## Independence

Niki ships its own implementation of the agent loop. The agent SDK and the ACP
runtime are vendored under `niki_agent/` with their MIT notices intact (see
`niki_agent/sdk/PROVENANCE.md` and `niki_agent/acp_sdk/PROVENANCE.md`); there is
no runtime dependency on an upstream distribution.

The old `dcode` and `deepagents-code` console scripts remain as deprecated
aliases for two minor versions, and `DEEPAGENTS_CODE_*` environment variables are
read as deprecated aliases of `NIKI_*`.

The user profile moves from `~/.deepagents` to `~/.niki` on first launch, with
the old directory migrated in place.

## Security model

By default Niki trusts the directory you run it in. Tool calls are gated by the
approval mode, but project files are read before any approval prompt. Do not run
Niki in a directory you do not trust without a sandbox backend; see
[`THREAT_MODEL.md`](THREAT_MODEL.md).

## Docs

- [`docs/niki/`](../../docs/niki) — product checklist, UI notes, evaluation
  protocol, and migration gates.
- [`AGENTS.md`](AGENTS.md) — contributor guide for this package.
- [`EXTENSIONS.md`](EXTENSIONS.md) — plugin manifest and extension API.

## License

MIT. Third-party copyright notices are retained in `/NOTICE` and `/LICENSE`;
vendored directories keep their license and provenance notes.
