# ◐ Niki Agent

> **Deprecated as the shipped product.** Niki Agent 2.0.0 is a self-contained
> Rust binary. This Python package stays in the repository as the reference
> implementation and the source of the behavioural fixtures; it is no longer
> installed by the release installer and no longer provides a `niki` console
> script. See the [repository README](../../README.md) for the product.

[![PyPI - Version](https://img.shields.io/badge/pypi-niki--agent-blue)](https://pypi.org/project/niki-agent/)
[![License: MIT](https://img.shields.io/badge/license-MIT-silver)](../../NOTICE)

Niki is a terminal coding agent with an interactive UI, persistent memory,
skills, subagents, and approval controls. It runs on any model that supports
tool calling.

**Installing the released product** — use the release installer in
[`install/`](../../install): it verifies the release checksum and installs into an
isolated environment. The script below builds from this checkout instead, which
is what you want when you are changing the code:

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
- **Sandboxes** — run tool calls in Daytona, Modal, Runloop, Vercel, Agentcore,
  or LangSmith sandboxes instead of on your machine.
- **Approval modes** — from approve-every-write to a reviewed auto mode.

## Independence

Niki ships its own implementation of the agent loop. The agent SDK and the ACP
runtime are vendored under `niki_agent/` with their MIT notices intact (see
`niki_agent/sdk/PROVENANCE.md` and `niki_agent/acp_sdk/PROVENANCE.md`), so
installing `niki-agent` pulls no second agent distribution.

`niki` is the only console script.

## Security model

By default Niki trusts the directory you run it in. Tool calls are gated by the
approval mode, but project files are read before any approval prompt. Do not run
Niki in a directory you do not trust without a sandbox backend; see
[`SECURITY.md`](../../SECURITY.md).

## Docs

- [`docs/niki/`](../../docs/niki) — architecture, independence guarantees, the
  keymap, and provider setup.
- [`AGENTS.md`](AGENTS.md) — contributor guide for this package.
- [`EXTENSIONS.md`](EXTENSIONS.md) — plugin manifest and extension API.

## License

MIT. Third-party copyright notices are retained in `/NOTICE` and `/LICENSE`;
vendored directories keep their license and provenance notes.
