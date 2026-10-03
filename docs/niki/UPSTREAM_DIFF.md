# Upstream Diff Log

Every file in this fork that is **not** new, and why it had to be touched.
`BASE_SHA` = `f57c6f383b7018024ca5cde2dc565048ea83202a`.

Anything not listed here is new and lives in the fork. Files listed here are
the entire merge cost against `langchain-ai/deepagents`.

Verify with:

```bash
git diff niki-base --stat          # everything the fork changed
git diff niki-base -- libs/deepagents --stat   # must be empty, always
```

## SDK package — never touched

`libs/deepagents` is upstream's SDK and is **off limits**. It is listed here so
that an empty diff is a positive, checkable claim rather than an omission.

| File | Reason |
| --- | --- |
| _(whole package)_ | Not modified. The final `git diff niki-base -- libs/deepagents` must be empty. |

## Modified upstream files

| File | Reason | Blast radius |
| --- | --- | --- |
| `libs/code/pyproject.toml` | Added three **dev-only** test dependencies to the existing `[dependency-groups] test`: `pytest-textual-snapshot` (visual regression at four terminal sizes — the checklist's `[S]` rows have no stdlib equivalent), `pexpect` (spawns the real console script under a pty for the lifecycle rows `run_test` structurally cannot reach), and `pyte` (a terminal emulator, so PTY assertions read a rendered screen instead of raw bytes). Also added one `filterwarnings` entry scoped to `pytest_textual_snapshot`: the plugin timestamps its diff SVGs with the deprecated `datetime.utcnow()`, and this repo treats warnings as errors, so a real snapshot mismatch would raise inside `save_svg_diffs` and crash `pytest_sessionfinish` before the diff could be printed. | Test-only. No runtime dependency added; no shipped code path changes. Additive `[dependency-groups]` and `filterwarnings` entries; nothing removed or relaxed. |

## Deliberately NOT patched, and why

These were the places a cheaper-looking fork would edit upstream. Each is
avoided for now, and each becomes a logged patch only if extension proves
impossible.

| Upstream file | Why we are not touching it yet |
| --- | --- |
| `deepagents_code/app.py` | 31,916 lines and the merge-cost hotspot. The Niki theme, keymap registry, and widget styling are reached through the theme registry and `register_theme()` instead. Any change here needs a measured reason. |
| `deepagents_code/tui/textual_adapter.py` | Hosts `execute_task_textual` and the stream loop. Presentation work targets the widgets and theme rather than the transport. |
| `deepagents_code/ui.py` (`show_help()`) | Hand-maintained and hardcodes `dcode`, so checklist row K1 (one keymap registry feeding footer, Help, and the slash popup) cannot be met by extension alone. **Known open patch site** — must be logged here with its reason when it happens. |
| `deepagents_code/update_check.py` | Niki must not phone home or point users at upstream's upgrade path. Handled without editing this file by policy, not by rewriting it. |
| `deepagents_code/tui/widgets/message_store.py` | The `WINDOW_SIZE=800` mount window is the prime suspect for the S3/S5 render-cost gap. `NikiMessageStore(MessageStore)` is the extension route; the window constants are class attributes a subclass can override. |

## Constraint that must hold at the end

```bash
git diff niki-base -- libs/deepagents   # must print nothing
```

If that is ever non-empty, the fork has violated its hard boundary regardless
of what the checklist says.