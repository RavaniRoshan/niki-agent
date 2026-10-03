# Niki Agent

A calm, fast terminal coding agent — a presentation-layer fork of
[Deep Agents](https://github.com/langchain-ai/deepagents).

Niki changes how the terminal agent *looks*, *feels*, and *responds*. It does
not change what the agent does: the tools, the permission defaults, the approval
semantics, and the `deepagents` SDK are upstream's, unmodified.

## What Niki changes

| | |
| --- | --- |
| **Command** | `niki` (new). `dcode` and `deepagents-code` still work, unchanged. |
| **Theme** | A Niki palette registered through upstream's existing theme registry. Dark and light, both contrast-measured. |
| **Idle cost** | 0 repaints in 5 s while idle, down from 10. |
| **Render cost** | Per-paint cost grows 1.2× from a short to a full transcript, down from 2.1×. |
| **Network** | Update checking, auto-update, and remote config fetch default **off**. |

## Install and run

```bash
cd libs/code
uv sync --all-groups

uv run niki                      # start Niki Agent
uv run niki --version            # Niki Agent 0.1.80, built on Deep Agents
uv run dcode                     # upstream's command, untouched
```

Niki needs the same provider credentials as upstream. Auth flows are unchanged.

## Documentation

| File | What it holds |
| --- | --- |
| [`docs/niki/CHECKLIST.md`](docs/niki/CHECKLIST.md) | Every checklist row with its real status. **Not all rows are met.** |
| [`docs/niki/BASELINE.md`](docs/niki/BASELINE.md) | Before/after measurements, with the commands that produced them. |
| [`docs/niki/DESIGN.md`](docs/niki/DESIGN.md) | The Phase 0 design note. |
| [`docs/niki/KEYMAP.md`](docs/niki/KEYMAP.md) | Generated from the live binding registry. Do not hand-edit. |
| [`docs/niki/UPSTREAM_DIFF.md`](docs/niki/UPSTREAM_DIFF.md) | Every upstream file this fork touches, and why. |
| [`docs/niki/OWNER_VERIFY.md`](docs/niki/OWNER_VERIFY.md) | Manual checks that need a real terminal and a real model key. |
| [`docs/niki/review/`](docs/niki/review/) | SVG frames of the current UI. |
| [`docs/niki/PROGRESS.md`](docs/niki/PROGRESS.md) | Build log. |

## Screenshots

SVG frames captured from the running app. These show **upstream's layout with
Niki's theme and title applied** — the visual redesign in Phase 3 is not
complete, and `docs/niki/CHECKLIST.md` says which rows that affects.

![First run at 80x24](docs/niki/review/test_first_run_snapshot[80x24].raw)

## Built on Deep Agents

Niki Agent is built on [Deep Agents](https://github.com/langchain-ai/deepagents)
by LangChain, MIT licensed. Upstream's copyright and license notices are
retained verbatim in `LICENSE` and `libs/code/LICENSE`. See [`NOTICE`](NOTICE)
for exactly what Niki does and does not change.

## Tests

```bash
cd libs/code
uv run pytest tests/unit_tests/niki/ -q    # the Niki harness
uv run ruff check deepagents_code/niki/ tests/unit_tests/niki/
uv run ty check deepagents_code/niki/ tests/unit_tests/niki/
```

No test requires a network, an API key, or a real terminal.