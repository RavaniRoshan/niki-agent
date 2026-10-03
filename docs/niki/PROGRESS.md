# Niki Agent — Progress Log

## Ground rules
- SDK package `libs/deepagents` is **never** modified. Final `git diff BASE_SHA -- libs/deepagents` must be empty.
- Local commits only. No push, no remote changes.
- Rebrand user-visible surfaces only. `dcode` untouched; new `niki` entry point.
- Every touched upstream file logged in `docs/niki/UPSTREAM_DIFF.md`.
- A row is WORKS only with a passing test or measured probe. Unverified is never reported as done.

## BASE_SHA
`f57c6f383b7018024ca5cde2dc565048ea83202a`
Clone: `https://github.com/RavaniRoshan/deepagents.git` → `/home/shiva/projects/nik-agent`, pinned detached at BASE_SHA, then branched `niki-agent`.
`libs/code` version at BASE_SHA: `deepagents-code` 0.1.80, SDK `deepagents` 0.7.21, textual 8.2.8, Python 3.12.3.

## Phase 0 — design note (COMPLETE)

### ✅ Completed
- ✅ Cloned the fork at BASE_SHA; branch `niki-agent` created.
- ✅ Installed `uv` 0.12.22 (was absent on this host); `uv sync --all-groups` in `libs/code`; `deepagents_code` imports clean.
- ✅ **Confirmed the current terminal agent is `libs/code` (`deepagents-code` 0.1.80, module `deepagents_code`). `deepagents-cli` does not exist** — the older doc reference is stale.
- ✅ Read the repo's own contributor docs: root `AGENTS.md`, `libs/DEVELOPMENT.md`, `libs/code/AGENTS.md`, `libs/code/ARCHITECTURE.md`, `libs/code/pyproject.toml`.
- ✅ Repo tooling identified and will be followed: **uv + make**, `ruff` lint, **`ty`** type check, `pytest`. No root `pyproject.toml`; work inside the package.
- ✅ Mapped 5 subsystems by read-only exploration (streaming/render, theming/glyphs, keys/help/slash, approvals/update, storage/lifecycle). Raw reports retained outside the repo.
- ✅ Measured baselines (below). Captured the full outbound-network audit.
- ✅ Wrote `docs/niki/DESIGN.md` (35 lines, under the 50-line cap).
- ✅ Located the existing fixture support `deepagents_code/_fake_models.py` — Phase 1 builds on it instead of authoring a second fake model.

### Baselines measured (pre-change)
| Probe | Command | Result |
| --- | --- | --- |
| CLI start, cold | `uv run dcode -v` | 0.349 s |
| CLI start, warm x5 | `uv run dcode -v` | 0.26 s |
| import, light | `python -c "import deepagents_code"` | 18.7 ms |
| import, app module | `python -c "import deepagents_code.app"` | 738.9 ms |
| import, heavy chain | `python -c "import langchain, langgraph, deepagents"` | **4546.8 ms** |
| environment | `uv run python -c "import textual;print(textual.__version__)"` | textual 8.2.8 |

### Key structural facts (from code-reading — UNVERIFIED until probed)
- Stream coalescing `_STREAM_FLUSH_INTERVAL = 0.1 s` → **10 renders/s**, below the 30–60 target. The per-chunk markdown re-parse is **already fixed** upstream (`messages.py:1451-1453`).
- Mount window `WINDOW_SIZE=800` / `HARD_WINDOW_SIZE=900` (`message_store.py:673`) is the prime suspect for S3/S5.
- `ui.show_help()` is **hand-maintained and hardcodes `dcode`** — K1 (one keymap registry) will need a real upstream patch.
- `app.py` is **31,916 lines** — the merge-cost hotspot.
- Niki theme is a **pure config addition** (`[themes.niki]`); no upstream stream change needed.

### Outbound network audit (pre-change) — full list in DESIGN.md §5
`update_check.py` PyPI calls; remote managed-config fetch `configuration/providers.py:66`; LangSmith trace (user opt-in). Update check **and** auto-update both **default ON** upstream; Niki must default both **OFF**, and must never surface `curl -LsSf https://langch.in/dcode | bash` or `uv tool install -U deepagents-code`.

### Open items needing the owner
- Runtime dependencies: none requested.
- Phase 1 dev-dependencies (justified): `pytest-textual-snapshot`, `pexpect`, `pyte`.
- Default change requiring sign-off: **auto-update ON → OFF for Niki** (mission-mandated; strictly less risk than upstream's posture).
- API key for the real-model smoke test — not yet requested; will ask when needed.

### Status
Phase 0 complete. **Stopped for design-note approval** before any UI change, per the Phase 0 gate.