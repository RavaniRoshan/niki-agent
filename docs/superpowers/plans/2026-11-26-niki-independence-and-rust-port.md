# Niki Agent — Independence, UI Parity, and the Rust Port

**Status:** planned 2026-11-26. Supersedes Tasks 6–7 of
`2026-11-24-niki-product-completion-dynamic-workflow.md`.

**Goal:** make Niki an independent product — own name, own module, own state
directory, no upstream runtime coupling — raise the terminal UI to the standard
the current Claude Code CLI sets (including accessibility), and then ship a
Rust implementation that starts with Linux/WSL.

**What this plan is not:** a rewrites-in-one-session plan. The Python product is
218k lines plus 227k lines of tests. The Rust port starts as a client that
speaks the protocol the agent engine already speaks, and the engine moves to
Rust slice by slice once the client has parity.

---

## Workflow status

| Slice | State | Evidence |
| --- | --- | --- |
| Phase 0 — delete the rig | in progress | deletions staged; `libs/code` check running |
| Phase 1 — rename | not started | — |
| Phase 1 — vendor SDK | not started | — |
| Phase 1 — trim workspace | not started | — |
| Phase 1 — brand gate | not started | — |
| Phase 2 — accessibility | not started | — |
| Phase 2 — parity rows | not started | — |
| Phase 3 — M0–M2 | not started | — |

## Owner decisions taken (2026-11-26)

1. **The Harbor/Claude Code comparison leaves the critical path.** It cannot run
   here (no container runtime, no hosted keys), and gating the port on it stalled
   the product. The harness is deleted; the protocol stays written down in
   `docs/niki/EVALUATION.md` for a later session.
2. **Local models leave the evidence set.** The Ollama runs measured model
   capability on this machine, not the CLI. The CLI smoke moves to a hosted
   free-tier model — Groq or the NVIDIA NIM free catalog — with the key supplied
   per session.
3. **Rust starts now, without a benchmark precondition.**
   `docs/niki/RUST_MIGRATION_GATE.md` now lists only product-quality gates.
4. **The product becomes independent of upstream Deep Agents**: fully renamed,
   rebranded, and detached internally and externally, while keeping the MIT
   attribution the license requires.

---

## Assumptions (defaults chosen; overturn any of these)

| # | Decision | Default | Why |
| --- | --- | --- | --- |
| A1 | `dcode` / `deepagents-code` console scripts | Keep as deprecation aliases for two minor versions, then delete | Users have them in shell history and scripts; a silent break is worse than a warning line. |
| A2 | The `deepagents` SDK | **Vendor** a pinned copy under `niki_agent/sdk/` with a provenance file | "Disconnected" has to mean no runtime import of an upstream package; a vendored copy keeps attribution visible and the agent loop ours. |
| A3 | Rust scope for the first release | Rust **client**, Python **engine** behind the existing LangGraph HTTP/SSE protocol | 218k lines of orchestration is not a client problem; the client is where latency and the UI live. |
| A4 | Platform order | Linux x86_64/aarch64 + WSL2 first, macOS arm64 second, Windows ConPTY third | Same order as the user's ask; ConPTY is the only target with a genuinely different terminal layer. |
| A5 | Hosted provider for the CLI smoke | Groq first (OpenAI-compatible endpoint), NVIDIA NIM second | One adapter shape covers both; whichever key the owner supplies first is the one that runs. |
| A6 | Repo shape | Keep `libs/code`; archive the other workspace packages | Deleting history helps nobody; the product tree becomes one package plus docs. |

---

## Phase 0 — Detach the comparison rig

Small, mechanical, and first, because everything else is easier with the noise
gone.

The rig is deleted as **one unit**, not surgically. `libs/evals` exists only to
host the harness; editing it in place and then archiving it in Phase 1 would
touch the same files twice. Its non-Harbor modules (`deepagents_evals`) are the
same package, so they leave with it.

| Step | Change | Verify |
| --- | --- | --- |
| 0.1 | Delete `libs/evals` and the workflows that exist only to drive it (`evals.yml`, `evals_trials.yml`, `unified_evals.yml`, `harbor.yml`, `_harbor_run.yml`, `_eval.yml`, `clbench.yml`, `niki-smoke.yml`) | no workflow references a deleted path |
| 0.2 | Delete `.github/scripts/evals/`, its tests, `prepare_local_dataset.py`, and its test | `.github/scripts/tests` passes |
| 0.3 | Remove the eval package from `ci.yml`, `release.yml`, `dependabot.yml`, `pre-commit`, label config, `SECRETS.md`, `LAYOUT.md`, `AGENTS.md`, `libs/README.md`, `libs/DEVELOPMENT.md`, and the openwiki eval page | `grep -ri "libs/evals\|harbor"` outside docs history returns nothing actionable |
| 0.4 | Rewrite `docs/niki/EVALUATION.md`, `docs/niki/evaluations/SMOKE.md`, and the gate doc to the deferral wording (already landed in `39a7a8c5`) | `readiness --phase python` still exits 0; `check_readme.py` consistent |

**Gate:** `make check` green in `libs/code`, `.github/scripts/tests` green
except the pre-existing environment failures, and no CI job references a
deleted path.

---

## Phase 1 — Independence

The rename is mechanical; the decisions are what make it safe.

### 1.1 Rename the product surface

| Surface | From | To | Compat |
| --- | --- | --- | --- |
| Import package | `deepagents_code` | `niki_agent` | none (internal) |
| Distribution | `deepagents-code` | `niki-agent` | one release with `Provides-Dist` note |
| Console scripts | `dcode`, `deepagents-code`, `niki` | `niki` | aliases print one deprecation line |
| Env vars (100) | `DEEPAGENTS_CODE_*` | `NIKI_*` | old names read as deprecated aliases, warn once |
| State dir | `~/.deepagents` | `~/.niki` | one-time migration with a marker file |
| Recovery header | `x-deepagents-recovery` | `x-niki-recovery` | accept both |
| Docs, URLs, `pyproject` metadata | LangChain / Deep Agents | Niki Agent | none |

`_env_vars.py` already centralizes every variable name, and
`tests/unit_tests/test_env_vars.py` fails on a bare string literal — that test
is the guardrail that makes the rename safe rather than a `sed` gamble.

### 1.2 Vendor the agent SDK

- Copy `deepagents==0.7.21` (the version pinned today) into
  `libs/code/niki_agent/sdk/`, preserving file headers.
- Add `niki_agent/sdk/PROVENANCE.md`: upstream repo, commit, license text,
  what was changed and why.
- Rewrite imports from `deepagents` to `niki_agent.sdk`; keep `langchain` /
  `langgraph` as ordinary third-party dependencies (they are libraries, not the
  product identity).
- Keep `NOTICE` and `LICENSE`; MIT attribution is a legal requirement, not a
  brand decision.

### 1.3 Trim the workspace

- Archive `libs/deepagents`, `libs/acp`, `libs/talon`, `libs/partners/*`,
  `libs/evals` — move to `archive/` in one commit (history already keeps them).
- Root `AGENTS.md`, `README.md`, CI workflows, and `libs/DEVELOPMENT.md`
  describe one package: `libs/code`.
- CI runs `libs/code` only; the matrix stays 3.12/3.13/3.14 on Linux, plus
  macOS for the install test.

### 1.4 Brand gate

A test, not a habit: scan user-facing strings, CLI help, notifications, and
docs for `deepagents`, `Deep Agents`, `dcode`, `LangChain`, and fail with the
file and line, allowing only `NOTICE`, `LICENSE`, `PROVENANCE.md`, and the
vendored file headers.

### 1.5 Release

Version `1.0.0`, own PyPI project `niki-agent`, `install.sh` pinned to the tag,
and a GitHub release with the wheel plus checksums.

**Gate:** `niki` installs from a clean clone under the new name, `readiness`
passes, `make check` is green, the brand gate passes, and `grep -r
deepagents_code libs/code/deepagents_code` finds nothing.

---

## Phase 2 — UI: accessibility and Claude Code CLI parity

Two goals in one phase because they touch the same components: the interface
should be as capable as the current Claude Code CLI **and** usable by people who
do not experience it the way its author does.

### 2.1 Parity inventory

Tracked as rows in `docs/niki/UI_PARITY.md`, each with evidence:

- streaming tool calls with collapse/expand; structured diffs with context
- todo/plan display that survives turns
- slash commands, `@` file mentions, `#` memory/instructions
- plan mode and approval prompts with a diff preview before write
- session picker, resume, transcript search, message copy
- paste handling (multi-line, large paste placeholder), image paste where the
  terminal supports it
- subagent/background task display with a stable activity line
- MCP server status and tool listing
- themes with a contrast-safe default, 16/256/truecolor paths
- keybinding registry with discoverable help

Rows come from the CLI's documented surface, not from memory: each row records
where the behaviour was observed (its docs or a captured session) before it is
implemented.

### 2.2 Accessibility bar

| Concern | Requirement | Test |
| --- | --- | --- |
| Colour is never the only signal | every status also has a glyph or word | snapshot + string assertion per state |
| Contrast | text ≥ 4.5:1, large text/glyphs ≥ 3:1, computed in every bundled theme at 16/256/truecolor | contrast test over the theme table |
| Colour depth | `NO_COLOR`, `NIKI_COLOR_DEPTH=16`, 256, truecolor all render legibly | existing V13 probe, extended to all themes |
| Glyphs | `NIKI_GLYPHS=ascii` covers every glyph, including new UI | registry-coverage test (fails on a raw literal) |
| Motion | `NIKI_MOTION=off` stops spinners and animation | snapshot with motion off |
| Screen readers | `NIKI_SCREEN_READER=1`: linear, append-only, no in-place line rewriting; events as lines | PTY run asserting no cursor-up escapes |
| Keyboard | every action reachable without a pointer; one keymap registry | keymap test enumerating every binding |
| Layout | 40, 80, 120 columns without clipping; 80×24 default | snapshot matrix |
| Language | plain words, no icon-only affordances, errors say what to do next | copy review list |

### 2.3 Work slices

1. Accessibility settings plumbing + tests (colour depth, glyphs, motion,
   reader mode) — the toggles exist today; make them complete and tested.
2. Parity rows that are missing, in order of user value: diff preview before
   write, todo display persistence, transcript search, session picker polish.
3. Contrast audit across every theme; fix the failures; record the table.
4. Reader mode end to end in a real PTY.

**Gate:** `UI_PARITY.md` has no `MISSING` row for a P0 feature; the
accessibility table is green; the PTY reader-mode run passes.

---

## Phase 3 — The Rust port

### 3.1 Architecture

```text
┌────────────── niki (Rust, ratatui) ───────────────┐
│ render, input, local state, slash palette,        │
│ approvals, diffs, sessions, themes, reader mode   │
└───────────────────────┬───────────────────────────┘
                        │ LangGraph HTTP/SSE protocol
                        ▼
┌──────────── engine (Python today) ────────────────┐
│ agent graph, tools, MCP, checkpoints, interrupts  │
└───────────────────────────────────────────────────┘
```

The protocol is the one `client/remote_client.py` already speaks through
`langgraph_sdk`/`RemoteGraph`: threads, runs, streaming modes, interrupts,
cancel. Rust implements the client half; nothing about the engine changes on day
one.

### 3.2 Crates

| Crate | Responsibility |
| --- | --- |
| `niki-proto` | protocol types, SSE framing, request builders; no I/O policy |
| `niki-engine-client` | connection, retries, cancel, thread/run lifecycle |
| `niki-render` | view model → buffer; theme tokens; glyph registry (mirrors `Glyphs`) |
| `niki-tui` | widgets, layout, keymap, focus, reader mode |
| `niki` | binary: argument parsing, engine spawn/attach, config, sessions |

### 3.3 Milestones and gates

| M | Deliverable | Gate |
| --- | --- | --- |
| M0 | Protocol capture: real SSE transcripts from the Python engine, normalized and committed as fixtures | fixtures replay deterministically; a schema test rejects unknown events |
| M1 | Read-only replay: `niki replay fixture.json` renders the transcript at 120×38, 80×24, 40×20 | text frames match the Python TUI snapshots for the same transcript, differences listed and justified |
| M2 | Live session: spawn engine, prompt, stream, tool cards, approvals, cancel, slash palette, session list | a scripted PTY session produces the same final transcript as the Python client for the same fixture model |
| M3 | Packaging: Linux tarball + `install.sh`; macOS arm64 build; WSL2 verified | clean-machine install, `niki --version`, first frame under budget |
| M4 | Windows: ConPTY path, PowerShell 5.1 and 7 | PTY scripted session on Windows CI |
| M5 | Engine slices in Rust behind the same protocol: tool execution, MCP client, session store | the Python engine can be swapped out per slice without a client change |

### 3.4 Performance gates (M3)

| Metric | Python today (measured) | Rust target |
| --- | --- | --- |
| First frame, warm | ~190 ms in-process, ~500 ms process start | ≤ 80 ms process start to first frame |
| Input echo p95 | 10.3 ms on a quiet box, 4.1 ms median (PTY) | ≤ 8 ms p95 at 120×40 |
| Resize storm, 100 resizes | 11.4 s headless (pause-bound) | ≤ 300 ms wall clock, no dropped frame |
| Idle CPU | probe-gated baseline | ≤ 0.5 % at idle |
| RSS | measured by probe | ≤ 60 MB with a 500-message transcript |
| Binary size | n/a | ≤ 25 MB stripped |

Numbers are recorded by the same probe shape the Python suite uses, so the two
implementations stay comparable.

### 3.5 Platform notes

- **Linux/WSL2**: the same binary. WSL needs no PTY special-casing; verify
  bracketed paste, OSC 52, and `/mnt/c` path handling.
- **macOS**: arm64 first; no ConPTY; verify key repeat and Option-as-Meta.
- **Windows**: ConPTY input differs (no true SIGWINCH, different escape
  handling); do it after parity, not alongside it.

---

## Work order

| # | Slice | Depends on | Evidence |
| --- | --- | --- | --- |
| 1 | Phase 0 deletions | — | `make check`, helper-script tests |
| 2 | Rename module + distribution + scripts | 1 | full suite + install test |
| 3 | Env vars + state dir migration | 2 | drift test, migration test |
| 4 | Vendor SDK + provenance | 2 | import graph test, license check |
| 5 | Trim workspace + CI | 3, 4 | one-package CI green |
| 6 | Brand gate | 5 | gate test |
| 7 | Accessibility slices | 2 | contrast/reader PTY tests |
| 8 | Parity rows | 7 | `UI_PARITY.md` |
| 9 | M0–M2 Rust client | 2 | fixture + PTY parity |
| 10 | M3 packaging | 9 | clean install on Linux/macOS/WSL |
| 11 | M4 Windows | 10 | Windows CI |
| 12 | M5 engine slices | 11 | per-slice swap test |

---

## Risks

- **License**: MIT attribution is mandatory. The brand gate must not strip
  `NOTICE`, `LICENSE`, or vendored headers.
- **Ecosystem**: skills, plugins, and MCP servers built against `dcode` expect
  `DEEPAGENTS_CODE_*` and `~/.deepagents`. The compat aliases are the mitigation;
  removing them is a later, announced break.
- **Protocol drift**: the LangGraph API is upstream and can change. The M0
  fixtures and schema test are what detect that early.
- **Scope creep in the rename**: a mechanical rename touching 266 files will
  conflict with any parallel work. Land it as one commit, not a trickle.
- **Windows**: ConPTY is the only place where "it works on Linux" is not
  evidence.

---

## Open questions for the owner

1. Do we keep the `dcode` alias past one release? (default: two minor versions)
2. Is the PyPI package name `niki-agent` acceptable, or do you want another?
3. Which hosted free-tier key lands first, Groq or NVIDIA NIM? (default: Groq,
   and the adapter covers both)
4. Should the archived `libs/*` stay in the repo under `archive/` or be deleted
   outright? (default: archive, because the license and provenance story is
   easier to audit)
