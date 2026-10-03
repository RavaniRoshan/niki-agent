# Niki Agent — Design Note (Phase 0)

BASE_SHA `f57c6f383b7018024ca5cde2dc565048ea83202a` · fork `RavaniRoshan/deepagents` (`main`).

## 1. Layout and the current terminal agent
Monorepo, no root `pyproject.toml`. `libs/{deepagents,code,acp,evals,talon,partners}`.
**`libs/code` is current**: dist `deepagents-code` 0.1.80, module `deepagents_code`.
`deepagents-cli` **no longer exists** — stale doc reference, confirmed. Scripts: `deepagents-code`, `dcode` → `deepagents_code:cli_main`. `dcode` stays untouched; Niki adds `niki`.
Tooling: **uv + make**; lint `ruff`; type check **`ty`** (0.0.61); tests `pytest`.
Sizes: `app.py` **31,916 L**, `tui/textual_adapter.py` 4,614, `update_check.py` 4,618, `theme.py` 925, `command_registry.py` 612.
Stream: `execute_task_textual` (`textual_adapter.py:1688`) → `agent.astream(stream_mode=["messages","updates","custom"], subgraphs=True, durability="exit")` (`:2106`) → adapter callbacks → `app.py:21224 _mount_message` under `_transcript_mutation_lock`. No custom Textual messages on the stream path.
Storage: `list[MessageData]` + index, sliding mount window `INITIAL_WINDOW_SIZE=30 / WINDOW_SIZE=800 / HARD_WINDOW_SIZE=900 / HYDRATE_BUFFER=8` (`message_store.py:673`). SQLite `~/.deepagents/.state/sessions.db`; history `history.jsonl`.

## 2. Baselines measured now
| Probe | Command | Result |
| --- | --- | --- |
| CLI start | `uv run dcode -v` | **0.349 s** cold, **0.26 s** warm |
| import, light | `python -c "import deepagents_code"` | **18.7 ms** |
| import, app | `python -c "import deepagents_code.app"` | **738.9 ms** |
| import, heavy | `python -c "import langchain, langgraph, deepagents"` | **4546.8 ms** |
Stream coalescing: `_STREAM_FLUSH_INTERVAL = 0.1 s` ClassVar on the message widgets → **10 renders/s** (below the 30–60 target). The per-chunk markdown re-parse is **already fixed** (`messages.py:1451-1453`) and must stay fixed.
No debouncer framework exists; no render cap anywhere. Prune debounce 0.2 s (0.001 s hard). Hydration coalesced per `call_later`.

## 3. First-pass checklist from code-reading — all UNVERIFIED
UNVERIFIED until probed: idle CPU/redraws; per-token cost at 100 vs 5000 msgs; memory growth; input-echo p95; first *frame* (not CLI exit); 10 MB tool output; resize storm. The 800-widget mount window is the prime suspect for S5 and likely the 10–30 ms/token floor.
Extension paths found: Niki theme = **pure config addition** (`[themes.niki]` → `ThemeColors` 17 fields, `theme.py:627-718`, `get_theme_colors` `:875`); `NikiMessageStore(MessageStore)` subclass needs no upstream edit except swapping `app.py:4535`; `shift+tab` reverse-nav is a structural `_SupportsReverseNav` protocol (`app.py:184-203`).

## 4. Upstream-diff strategy and riskiest places
Extension order: (1) new `niki` script → new module; (2) Niki theme via registry; (3) new `.tcss`; (4) widget subclasses; (5) patch upstream only when impossible. Every patched upstream file logged in `docs/niki/UPSTREAM_DIFF.md`. `libs/deepagents` never touched.
Riskiest: **`ui.show_help()` is hand-maintained and hardcodes `dcode`** — K1's single-registry requirement forces a real patch here. `app.py` at 32K lines is the merge-cost hotspot. `deepagents_code/extensions/api.py` exposes no presentational hook (middleware/tool/backend_route/shutdown only), so the extension host cannot be used.

## 5. Outbound network (audit, pre-change) — all must be off/opt-in for Niki
`update_check.py` → `https://pypi.org/pypi/deepagents-code/json` (`_version.py:10`) at `:731`, `:873`; SDK `…/deepagents/json` (`:13`) at `:1138`; `timeout=3`, 24 h cache. **Update check and auto-update both default ON** — Niki defaults both **OFF**. Must not surface `INSTALL_SCRIPT_COMMAND = "curl -LsSf https://langch.in/dcode | bash"` (`:233`) or `upgrade_command()` → `uv tool install -U deepagents-code`. Remote managed-config fetch `configuration/providers.py:66` (1 MB, 5 s) — off unless explicitly configured. LangSmith trace is user-opt-in; `auto_mode.py:1094` only reads the current run tree. No telemetry in `update_check.py`.

## 6. Needed from you
Runtime deps: none requested. Dev-deps at Phase 1: `pytest-textual-snapshot`, `pexpect`, `pyte` (justified). One default change requiring sign-off: **auto-update ON → OFF for Niki** (mission mandates it; strictly less risk than upstream). No API key yet.