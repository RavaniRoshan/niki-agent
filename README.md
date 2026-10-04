<div align="center">

# ◐ Niki Agent

*Calm, fast, keyboard-native. A terminal coding agent that gets out of the way.*

[![Python](https://img.shields.io/badge/python-3.12%2B-blue?style=flat-square)](https://www.python.org)
[![Textual](https://img.shields.io/badge/terminal-Textual%208-5c873a?style=flat-square)](https://textual.textualize.io)
[![License: MIT](https://img.shields.io/badge/license-MIT-silver?style=flat-square)](NOTICE)

[Install](#install) · [Run it](#run-it) · [What changed](#what-changed) · [Themes](#themes) ·
[Providers](#providers) · [Docs](#docs)

</div>

<div align="center">
  <img src="docs/niki/review/test_first_run_snapshot[120x38].raw" width="900" alt="Niki Agent first-run screen">
</div>

Niki Agent is a presentation-layer fork of [Deep Agents](https://github.com/langchain-ai/deepagents).
It keeps the agent, the tools, the permissions, and the SDK exactly as they ship —
and rebuilds everything you look at and feel while you use them.

```bash
cd libs/code
uv sync --all-extras
uv run niki
```

> [!TIP]
> No API key is needed to try the interface. `niki` boots straight to a working
> composer. Set a key only when you want it to actually do something:
> `export ANTHROPIC_API_KEY=sk-ant-...`

---

## Install

Requires Python 3.12+ and [uv](https://docs.astral.sh/uv/).

```bash
git clone https://github.com/RavaniRoshan/niki-agent.git
cd niki-agent/libs/code
uv sync --all-extras
```

Install just one provider's extra instead of all of them with
`uv sync --extra anthropic`.

## Run it

```bash
uv run niki                      # interactive
uv run niki --help               # generated from the live key bindings
uv run niki --version            # Niki Agent 0.1.80, built on Deep Agents
uv run dcode                      # upstream's command, unchanged
```

## What changed

Nothing about *what the agent does*. Everything about *what it feels like*.

| | Before | After |
| --- | --- | --- |
| Idle redraws over 5 s | 10 | **0** |
| Render cost, worst case vs. a short transcript | 2.09× | **1.21×** |
| Resident memory, long session | 396 MB | **255 MB** |
| Composer | — | a single rule, not a box |
| Update check / auto-update | on | **on**, with `NIKI_DISABLE_*` opt-outs |

- **Still.** A new command, `niki`, beside the untouched `dcode`.
- **A theme layer.** Registered through the existing theme registry, not by
  forking it. Every colour comes from one module.
- **Its own motion.** A ping-ponging `◐◓◑◒` sweep with adaptive cadence, and
  `NIKI_REDUCED_MOTION` for anyone who wants stillness.
- **A bounded mount window.** Keeps a long transcript from dragging the render
  cost with it.

> [!NOTE]
> `dcode` and `deepagents-code` still resolve to upstream's entry point and
> behave identically. `niki` is additive.

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
NO_COLOR=1 uv run niki                     # colour off
UI_CHARSET_MODE=ascii uv run niki          # ASCII glyph fallback
NIKI_REDUCED_MOTION=1 uv run niki          # no animation
```

> [!WARNING]
> The palette draws direction from Codex and Kimi Code, but copies **no** colour
> values from either. A test asserts that against every hex value those tools
> use, so "inspired by" cannot quietly become "reproduced".

## Providers

Nine ship already — Anthropic, OpenAI, Google, Ollama, OpenRouter, NVIDIA, Groq,
Together, DeepSeek. Three more are configured rather than compiled in:

```toml
# ~/.deepagents/config.toml
[providers.kimi]
display_name = "Kimi"
class_path = "langchain_openai:ChatOpenAI"
base_url = "https://api.moonshot.ai/v1"
api_key_env = "MOONSHOT_API_KEY"
models = ["kimi-k3", "kimi-k2.7-code", "kimi-k2.6"]
```

Full details, caveats, and the Kilo and OpenCode Zen blocks are in
[`docs/niki/PROVIDERS.md`](docs/niki/PROVIDERS.md). None of the three needs a new
runtime dependency.

## Docs

| | |
| --- | --- |
| [`CHECKLIST.md`](docs/niki/CHECKLIST.md) | Every row with its real status. Not all of them are met. |
| [`BASELINE.md`](docs/niki/BASELINE.md) | Before/after numbers and the commands behind them. |
| [`OWNER_VERIFY.md`](docs/niki/OWNER_VERIFY.md) | What still needs a real terminal and a real key. |
| [`KEYMAP.md`](docs/niki/KEYMAP.md) | Every key binding, generated from the registry. |
| [`PROVIDERS.md`](docs/niki/PROVIDERS.md) | Provider setup and gateway notes. |
| [`UPSTREAM_DIFF.md`](docs/niki/UPSTREAM_DIFF.md) | Every upstream file this fork touches, and why. |
| [`PROGRESS.md`](docs/niki/PROGRESS.md) | Build log, including the turns that were wrong. |

**Status: 26 WORKS · 33 PARTIAL · 11 MISSING** of 76 checklist rows. The rows
still open are listed in the checklist rather than smoothed over here — most are
visual work that needs a real terminal to judge, not another test.

## Tests

```bash
cd libs/code
uv run pytest tests/unit_tests/niki/ -q    # no network, no API key, no terminal
```

## Built on Deep Agents

Built on [Deep Agents](https://github.com/langchain-ai/deepagents) by LangChain,
MIT licensed. Upstream's copyright and notices are retained verbatim — see
[`NOTICE`](NOTICE). Niki changes presentation and responsiveness; it does not
change agent behaviour, the tool set, or the permission defaults.

> [!NOTE]
> The screenshot above shows upstream's layout with Niki's theme, title, and
> motion applied. The full visual redesign is in progress, and
> [`CHECKLIST.md`](docs/niki/CHECKLIST.md) says exactly which parts.