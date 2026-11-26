# Niki Product Completion Dynamic Workflow Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish one durable workflow that closes Niki's remaining Python readiness gaps, runs the matched Claude Code evaluation only after product readiness, and then carries the same verified behavior through the gated Rust migration.

**Architecture:** Keep `docs/niki/CHECKLIST.md` as the source of product requirements and evidence; add a small dynamic queue/controller plus a reusable repo skill that turns the next open checklist item into one testable task card. The controller records ownership, blockers, commits, and evidence in one state artifact, while the skill enforces tests and independent verification. The matched benchmark is a final Python readiness gate before Rust and is repeated after Rust parity; no early or partial Claude Code comparison is presented as a product result.

**Tech Stack:** Existing Python `>=3.12,<4.0`, LangGraph/LangChain, Textual, Harbor 0.25.0, Terminal-Bench 4.1.0, uv, Make, pytest, Ruff, `ty`, and Rust/Cargo after the Python gate.

**Spec:** `docs/superpowers/plans/2026-11-23-niki-agent-architecture.md`; `docs/niki/CHECKLIST.md`; `docs/niki/RUST_MIGRATION_GATE.md`; `docs/niki/EVALUATION.md`; repository `AGENTS.md`; the supplied harness architecture screenshots.

## Global Constraints

- Keep the working product in Python until every Python readiness gate in `docs/niki/RUST_MIGRATION_GATE.md` passes.
- Keep `libs/deepagents` unchanged. Verify `git diff f57c6f383b7018024ca5cde2dc565048ea83202a -- libs/deepagents` is empty at every release gate.
- Preserve `dcode` and `deepagents-code` behavior; ship Niki behind the separate `niki` entry point.
- Keep the modular-monolith dependency direction: domain and application contracts stay framework-free; LangGraph, ACP, Textual, persistence, provider, and execution integrations stay in outer adapters.
- Reuse existing session/checkpoint, provider, workspace, permission, MCP, hooks, skills, memory, goals, subagent, and QuickJS implementations where verified behavior already exists.
- Do not add runtime dependencies without a demonstrated requirement and a maintenance reason. Use in-memory adapters for use-case tests; core tests must not need Docker, network access, or a model key.
- Keep each work item to one owner and one independently testable slice. Do not create another top-level implementation plan; the workflow skill creates a task card from the live checklist at each iteration.
- Do not mark an item `WORKS` without a passing test, measured probe, real-terminal observation, or approved owner-verification evidence appropriate to that item.
- Do not silently drop P1 checklist items; before a readiness gate, each P1 item is `WORKS` or an owner-approved `DEFERRED` entry with a recorded reason.
- Use only these workflow states: `WORKS`, `PARTIAL`, `MISSING`, `BROKEN`, `NOT STARTED`, `BLOCKED`, and owner-approved `DEFERRED`; retain each checklist row's existing P0/P1 priority and evidence text.
- Bundle owner-only decisions into one review packet. Never silently weaken a target or convert an unverified row to `WORKS` to satisfy a gate.
- Never put API keys in source, command logs committed to the repository, workflow artifacts, or benchmark reports. Report credential availability as set/unset only.
- Do not run a Claude Code comparison until the Python product readiness gate is met. A smoke is only a run-validity check; it is not a score or evidence of competitiveness.
- Use Harbor 0.25.0, dataset `terminal-bench/terminal-bench@4.1.0`, Niki `langgraph` graph `dcode`, Claude Code `claude-code`, model `anthropic:claude-opus-5-5`, reasoning effort `max`, LangSmith sandbox/plugin, timeout multiplier `1.0`, three attempts per task, one concurrent trial, and zero retries for both matched runs.
- Claim “competitive with Claude Code” only if Niki resolution rate is at least 55% and the lower bound of the paired 95% confidence interval is at least -10 percentage points.
- Label the missing historical baseline honestly. A new measurement is `first post-Task-11 baseline`; it cannot reconstruct the missed pre-Task-1 baseline.
- Install and use the requested architecture skill with `npx skills add https://github.com/wshobson/agents --skill architecture-patterns`; keep the installed skill pinned in the repository's skill lock.

## Review Focus

- A stale checklist row or missing evidence is treated as complete. Pin this with `test_readiness_requires_evidence_for_every_p0` in Task 2.
- A blocked P0 disappears from the queue or is silently skipped at the readiness gate. Pin this with `test_blocked_p0_remains_open_until_evidence_is_recorded` in Task 2.
- Two workers claim overlapping changes in the shared worktree. Pin this with `test_workflow_allows_only_one_active_item` in Task 2.
- A provider context limit or missing sandbox credential is mislabeled as an agent failure or a passing eval. Pin this with `test_blocked_p0_remains_open_until_evidence_is_recorded` and `test_evaluation_gate_rejects_missing_or_incomplete_artifacts` in Task 2.
- Rust work starts before Python tests, owner checks, and the matched benchmark gate pass. Pin this with `test_rust_phase_requires_python_readiness_gate` in Task 2.

## File Structure

- `libs/code/scripts/niki_workflow.py` owns checklist parsing, dynamic next-item selection, workflow state transitions, and readiness-gate output.
- `libs/code/tests/unit_tests/test_niki_workflow.py` owns queue, evidence, ownership, and phase-transition behavior tests; `libs/code/tests/fixtures/niki-workflow/` owns small checklist/state fixtures.
- `docs/niki/workflow-state.json` stores only orchestration state: current phase, active item/step, saved per-item steps, owner/branch, blocked items, latest eval command/result and numeric cost when available, last commit, and evidence references. Product requirements and item status remain in `docs/niki/CHECKLIST.md`.
- `.agents/skills/niki-product-completion/SKILL.md` defines the reusable task-card loop, test/review/handoff protocol, and stop conditions for coding agents.
- `docs/niki/CHECKLIST.md` remains the product acceptance matrix, with separate Python and Rust status/evidence columns; `docs/niki/PROGRESS.md` records current work and session handoffs; `docs/niki/OWNER_VERIFY.md` remains the bundled real-terminal checklist.
- `docs/niki/EVALUATION.md`, `docs/niki/evaluations/SMOKE.md`, `docs/niki/evaluations/BASELINE.md`, and `docs/niki/evaluations/terminal-bench-baseline-comparison.json` own evaluation protocol and observed evidence.
- `docs/niki/RUST_MIGRATION_GATE.md` owns the transition between the Python and Rust phases; the dynamic workflow does not start Rust before its listed gates pass.

---

### Task 1: Close the current local evaluation-harness gaps

**Files:**
- Modify: `libs/evals/scripts/compare_niki_terminal_bench.py`
- Modify: `libs/evals/tests/unit_tests/test_niki_competitor_eval.py`
- Modify: `docs/niki/evaluations/SMOKE.md`
- Verify: `.github/workflows/niki-smoke.yml`

**Interfaces:**
- Preserve `load_trial_results(job_dir: Path) -> list[TrialResult]` and `compare_results(niki: Sequence[TrialResult], claude_code: Sequence[TrialResult], *, bootstrap_samples: int = 10_000, seed: int = 20261123) -> ComparisonReport`.
- Preserve the result and protocol dataclasses and JSON field names in `docs/superpowers/plans/2026-11-24-niki-benchmark-baseline-repair.md`; add required CLI option `--niki-agent langgraph` and emit `protocol.niki_agent = "langgraph"`.

- [ ] **Step 1: Add the missing evaluator contract tests**

  Extend `test_load_trial_results_rejects_invalid_records` to cover malformed JSON, duplicate trial IDs/names, infinite and out-of-range rewards, negative duration/token/cost, and missing reward with and without execution `exception_info`. Add tests for two/four attempts, duplicate attempt indexes, cross-agent checksum mismatch, linear-interpolated duration percentiles (`[0, 10, 20, 30]` gives median `15` and p95 `28.5`), and the complete comparison JSON protocol/task shape including `protocol.niki_agent == "langgraph"`. Use specific exception matches so a test cannot pass on an unrelated `ValueError`.

  Expand `test_harbor_runner_targets_use_same_terminal_bench_inputs` to assert the exact `--agent` values, Niki `graph=dcode`, LangSmith `--env` and `--plugin`, dataset/plugin arguments, Claude Code `permission_mode=bypassPermissions`, timeout, retry, three attempts, separate output paths, and smoke-only `--n-tasks 2` behavior. Compare parsed shared arguments between both `make -n` outputs.

- [ ] **Step 2: Run the added tests to expose the remaining gaps**

  Run:

  ```bash
  cd libs/evals
  uv run pytest tests/unit_tests/test_niki_competitor_eval.py -q
  ```

  Expected: the newly added invalid-input and report assertions fail until implemented; preserve their failure output for review.

- [ ] **Step 3: Keep comparator helpers focused and resolve lint findings**

  Split `_parse_trial_record`, `load_trial_results`, and `compare_results` into focused helpers consistent with the repository guidance to keep functions about 20 lines or less. Keep every exception trial at reward `0.0`; verify every attempt checksum within an agent and across agents; require nonempty job IDs; require `--niki-agent langgraph` for this Python comparison. Fix Ruff findings in the changed comparator and tests without file-wide ignores or broad exception catches.

- [ ] **Step 4: Remove stale smoke text and run the local eval gate**

  Remove the literal `EOF` line from `docs/niki/evaluations/SMOKE.md`. Keep the SDK smoke labeled SDK-level and the `niki -n`/Harbor checks labeled `NOT RUN` with their actual blockers. Verify the workflow parses as YAML and all `uses:` references are full 40-character SHAs.

  Run:

  ```bash
  cd libs/evals
  uv lock --check
  make lint
  uv run pytest tests/unit_tests/test_niki_competitor_eval.py tests/unit_tests/test_harbor_langgraph_agent.py tests/unit_tests/test_harbor_langsmith_integration.py -q
  uv run harbor --version
  cd ../..
  git diff --check
  git diff f57c6f383b7018024ca5cde2dc565048ea83202a -- libs/deepagents
  ```

  Expected: lint and tests pass, Harbor reports `0.25.0`, and the SDK diff is empty. Do not attempt a live Harbor job in this task.

- [ ] **Step 5: Commit the evaluator repair**

  ```bash
  git add libs/evals/scripts/compare_niki_terminal_bench.py libs/evals/tests/unit_tests/test_niki_competitor_eval.py docs/niki/evaluations/SMOKE.md
  git commit -m "fix(evals): complete local benchmark verification"
  ```

---

### Task 2: Build the reusable dynamic completion workflow

**Files:**
- Create: `libs/code/scripts/niki_workflow.py`
- Create: `libs/code/tests/unit_tests/test_niki_workflow.py`
- Create: `libs/code/tests/fixtures/niki-workflow/checklist.md`
- Create: `libs/code/tests/fixtures/niki-workflow/state.json`
- Create: `docs/niki/workflow-state.json`
- Create: `.agents/skills/niki-product-completion/SKILL.md`
- Modify: `docs/niki/CHECKLIST.md` to add separate Python and Rust evidence columns
- Modify: `docs/niki/PROGRESS.md`
- Install: `.agents/skills/architecture-patterns/` via `npx skills add https://github.com/wshobson/agents --skill architecture-patterns`

**Interfaces:**
- `WorkflowStatus = Literal["WORKS", "PARTIAL", "MISSING", "BROKEN", "NOT STARTED", "DEFERRED"]` and `Priority = Literal["P0", "P1"]`; blocked state lives in `WorkflowState.blocked`, not in checklist status cells.
- `WorkItem(item_id: str, title: str, priority: Priority, python_status: WorkflowStatus, python_evidence: str, rust_status: WorkflowStatus, rust_evidence: str, source_row: str)` is parsed from the existing checklist table.
- `WorkflowState(phase: Literal["python", "python_evaluation", "rust", "rust_evaluation", "complete"], active_item: str | None, active_step: str | None, item_steps: dict[str, str], owner: str | None, branch: str | None, blocked: dict[str, str], last_commit: str | None, last_eval: dict[str, object] | None, completed: dict[str, dict[str, str]])` is serialized to `docs/niki/workflow-state.json`.
- `parse_checklist(text: str) -> list[WorkItem]`, `select_next_item(items: Sequence[WorkItem], state: WorkflowState) -> WorkItem | None`, and `readiness(items: Sequence[WorkItem], phase: Literal["python", "rust"]) -> list[str]` are pure functions. Parse every item table with the existing `Row | P | Status | Evidence | Notes` headers; ignore the palette, metrics, and other non-item tables.
- `evaluation_readiness(*, smoke: dict[str, object], comparison: dict[str, object] | None, baseline_avg_at_3: float | None, final_avg_at_3: float | None, phase: Literal["python", "rust"]) -> list[str]` rejects absent smoke evidence, absent comparison data, mismatched protocol values, non-paired task sets, any task without exactly three attempts per agent, a failed competitiveness threshold, or unified avg@3 more than 5 percentage points below baseline.
- CLI commands are `status`, `next`, `start --id ID --owner NAME --branch BRANCH`, `step --id ID --name TEXT`, `block --id ID --reason TEXT`, `unblock --id ID --resolution TEXT`, `finish --id ID --commit SHA --evidence TEXT`, `readiness --phase python|rust`, `evaluation-readiness --phase python|rust`, `advance --to PHASE`, and `reopen --to python|rust --reason TEXT`.
- `next` resumes the current active item and its recorded step if one exists; otherwise it chooses the first unblocked, unresolved P0 item for the active phase in checklist order, then unresolved P1 items, and restores that item's saved step. It presents blocked items in status output and never treats them as complete. It returns the phase readiness gate only when all P0 items are `WORKS` with evidence and each P1 item is `WORKS` or owner-approved `DEFERRED` with a reason.
- `finish` requires the checklist row to be `WORKS`, a nonempty commit hash, and nonempty evidence. The state file tracks the proof reference; it does not replace the evidence in `CHECKLIST.md`.
- `select_next_item` resumes the current active item, or returns `None` in `python_evaluation`, `rust_evaluation`, or `complete`; those phases expose only their gate/handoff action.
- `step` updates only the active item's saved step. `block` releases the active claim, if any, so independent items can proceed while preserving that step; `unblock` requires a nonempty resolution note and leaves the checklist item unresolved until its evidence passes. `advance` enforces `python → python_evaluation → rust → rust_evaluation → complete`, requiring the prior phase gate to pass and no active item. `reopen` returns to the relevant implementation phase after a failed benchmark and records the reason; a failing evaluation cannot be marked complete.
- CLI exit codes are `0` for a valid action or passed gate, `1` for a failed readiness/evaluation gate, and `2` for invalid arguments or state transitions.
- `finish` requires the full 40-character commit SHA to resolve to a Git commit.

- [ ] **Step 1: Write workflow state and selector tests**

  Add `test_parse_checklist_preserves_phase_status_and_evidence`, `test_next_selects_first_open_p0_before_p1`, `test_next_resumes_active_item_without_claiming_another`, `test_step_updates_only_active_item`, `test_blocked_p0_remains_open_until_evidence_is_recorded`, `test_unblock_requires_resolution_and_keeps_item_unresolved_until_retest`, `test_workflow_allows_only_one_active_item`, `test_finish_requires_works_commit_and_evidence`, `test_readiness_requires_evidence_for_every_p0`, `test_readiness_requires_p1_work_or_approved_deferral`, `test_evaluation_gate_rejects_missing_or_incomplete_artifacts`, `test_rust_phase_requires_python_readiness_gate`, and `test_failed_evaluation_reopens_implementation_phase`. Give the fixture rows these IDs and values: `P0-WORKS` (Python WORKS with `tests/test_ok.py::test_ok`, Rust NOT STARTED), `P0-FIRST` (Python PARTIAL), `P0-SECOND` (Python MISSING), `RUST-P0` (Python WORKS with evidence, Rust NOT STARTED), `P1-OPEN` (Python PARTIAL), `P1-DEFERRED` (Python DEFERRED with owner decision evidence), and `P0-BLOCKED` (Python PARTIAL plus a state blocker). In the selector fixture without `P0-BLOCKED`, assert `next` selects `P0-FIRST` before `P0-SECOND` and `P1-OPEN`, then selects `P1-OPEN` after both P0 rows pass; in the blocker fixture assert blocking active `P0-FIRST` releases its claim and `next` selects `P0-SECOND`, readiness remains exit `1`, and unblocking requeues unresolved `P0-FIRST` with its saved step. Also assert active-item resume uses its saved step, invalid second claims and phase transitions exit `2`, gates pass with exit `0` only with required evidence, and Rust cannot advance before the Python gate.

- [ ] **Step 2: Run the new workflow tests and confirm they fail**

  Run:

  ```bash
  cd libs/code
  uv run pytest tests/unit_tests/test_niki_workflow.py -q
  ```

  Expected: FAIL because the workflow state types and commands do not exist.

- [ ] **Step 3: Implement the pure selector and state transitions**

  Change each product item table header to `| Row | P | Python Status | Python Evidence | Rust Status | Rust Evidence | Notes |`; move the current status/evidence into the Python columns, set each Rust status to `NOT STARTED`, and leave Rust evidence blank. Implement the interfaces above using only the standard library. Parse every item table by its header; do not copy requirements into a second source of truth. Reject unknown item IDs, a second simultaneous active item, `finish` without a `WORKS` status/evidence/full commit SHA, a phase advance before its preceding gate passes, and a completion transition after a failed evaluation. Keep `status` and `next` read-only.

- [ ] **Step 4: Write the reusable agent workflow skill**

  In `.agents/skills/niki-product-completion/SKILL.md`, document the dynamic harness contract:

  - **Objective:** finish the live product backlog and final evaluation; do not claim perfection, skip blocked work, or start Rust before the gate.
  - **Inputs:** current branch, `CHECKLIST.md`, `PROGRESS.md`, `OWNER_VERIFY.md`, architecture plan, relevant package `AGENTS.md`, and workflow state.
  - **Task card:** item ID/title, exact source requirement, files, test name and command, acceptance result, owner/branch, dependencies, and known blocker.
  - **Loop:** discover current state; select one eligible item; write a failing behavior test; implement one slice; run focused test/lint/type checks; obtain an independent review; update checklist/progress/state; commit; return the evidence and next item.
  - **Status view:** show the current phase, queue, active task/step, gate status, blocker or latest evaluation result, and handoff fields.
  - **Stop conditions:** failing checks, overlapping ownership, unresolved product decision, unavailable owner-only evidence, missing credentials/runtime at the final eval, or any attempted `libs/deepagents` change.
  - **Handoff:** report status, item ID, commit, changed files, exact checks and results, evidence links, blocked items, and next item. Never create another top-level plan for an item generated by this loop.

  Check the repository first; install the architecture-patterns skill with the command in Files if it is absent, read its `SKILL.md`, and retain the resolved source revision in `skills-lock.json`. Use its inward dependency rule and in-memory adapter guidance when a backlog item changes a port or adapter.

- [ ] **Step 5: Verify workflow commands and the handoff loop**

  Run:

  ```bash
  cd libs/code
  uv run pytest tests/unit_tests/test_niki_workflow.py -q
  uv run python scripts/niki_workflow.py status
  uv run python scripts/niki_workflow.py next
  ```

  Expected: tests pass; `status` reports current P0/P1 counts and blockers; `next` returns a task card for the first open P0 row without changing files.

- [ ] **Step 6: Commit the workflow controller and skill**

  ```bash
  git add libs/code/scripts/niki_workflow.py libs/code/tests/unit_tests/test_niki_workflow.py libs/code/tests/fixtures/niki-workflow docs/niki/workflow-state.json docs/niki/PROGRESS.md .agents/skills/niki-product-completion .agents/skills/architecture-patterns skills-lock.json
  git commit -m "feat(code): add dynamic Niki completion workflow"
  ```

---

### Task 3: Reconcile the live product checklist and record a current regression baseline

**Files:**
- Modify: `docs/niki/CHECKLIST.md`
- Modify: `docs/niki/PROGRESS.md`
- Modify: `docs/niki/BASELINE.md` only with measured evidence
- Update: `docs/niki/workflow-state.json`

**Interfaces:**
- The workflow controller from Task 2 reads `CHECKLIST.md`; it does not infer completion from old plan checkboxes or stale progress summaries.
- A checklist `WORKS` row names a test/probe and exact passing evidence; an owner-only observation remains `PARTIAL` until the owner records it.

- [ ] **Step 1: Re-audit the current branch before selecting product work**

  For each P0 row, inspect the current implementation and cited test/probe. Re-run the narrow evidence command for rows whose evidence predates the current branch. Reconcile disagreements between `CHECKLIST.md`, `PROGRESS.md`, implementation, and recent commits. Refresh the tally from the current branch; do not reuse the old `26 WORKS · 33 PARTIAL · 11 MISSING` count without recounting.

- [ ] **Step 2: Record one non-comparative baseline before new behavior changes**

  If the required provider and LangSmith credentials are available, run the existing unified eval on a clean, pushed commit before any product behavior changes. Record the exact source SHA and confirm the workflow run evaluated that SHA:

  ```bash
  gh workflow run unified_evals.yml --ref <branch> \
    -f models=anthropic:claude-opus-5-5 \
    -f categories=autonomous \
    -f agent_impls=dcode \
    -f profile=lite \
    -f rollouts=3 \
    -f n_retries=0 \
    -f agent_timeout_multiplier=1.0 \
    -f sandbox_env=langsmith
  ```

  Label it `current Python completion baseline`; it is an internal regression reference, not the missed pre-Task-1 baseline and not a Claude Code comparison. If credentials are unavailable, record `NOT RUN` and keep the regression gate blocked; do not invent a score.

- [ ] **Step 3: Bundle requirement decisions that block the P0 gate**

  Create one owner decision packet in `docs/niki/PROGRESS.md` listing each P0 target whose current metric or measurement cannot meet its written criterion (including evidence, the measurable limitation, and a recommended keep/change/defer decision). Do not edit a threshold or downgrade priority until the owner records the decision. Continue with independent P0 items while a decision is pending.

- [ ] **Step 4: Commit the reconciled state**

  Run `cd libs/code && uv run pytest tests/unit_tests/test_niki_workflow.py -q` and `git diff --check`, then commit only the evidence/status changes:

  ```bash
  git add docs/niki/CHECKLIST.md docs/niki/PROGRESS.md docs/niki/workflow-state.json docs/niki/BASELINE.md
  git commit -m "docs(niki): reconcile product readiness evidence"
  ```

---

### Task 4: Drive the remaining Python P0 backlog through dynamic task cards

**Files:**
- Modify: files owned by the selected checklist item only
- Test: the exact test/probe assigned to the selected task card
- Update: `docs/niki/CHECKLIST.md`, `docs/niki/PROGRESS.md`, `docs/niki/workflow-state.json`

**Interfaces:**
- Consumes `WorkItem`, `select_next_item`, and task-card/handoff format from Task 2.
- Produces one independently verified commit per completed checklist item; an item moves to `WORKS` only after its specified evidence passes.

- [ ] **Step 1: Select and claim one open P0 item**

  From `libs/code`, run `uv run python scripts/niki_workflow.py next`, review the emitted card against the current source row and package `AGENTS.md`, then claim it with `start --id ID --owner <agent> --branch <branch>`. If the row has hidden dependencies or ambiguous acceptance, add those facts to its card before editing. Do not launch overlapping workers on the shared worktree.

- [ ] **Step 2: Implement one behavior with test-first verification**

  Write or strengthen the exact regression test named by the card; run it and capture the failure; make the smallest change in the owning module; run that test, its related package tests, Ruff, and `ty` for changed Python files. Use the architecture-patterns skill only to clarify a demonstrated boundary; do not add a layer that has no tested behavior to own.

- [ ] **Step 3: Review evidence and update the dynamic queue**

  Independently inspect the diff, run the task's focused check, verify the SDK ownership diff remains empty, and confirm any external effect stays behind the existing adapter/sandbox boundary. Update the checklist row with the exact test/probe and result; call `finish --id ID --commit SHA --evidence TEXT` only after those updates. Commit the item before selecting the next task.

- [ ] **Step 4: Repeat until the Python readiness selector reports no open P0 work**

  Repeat Steps 1–3 for each open P0. If a row requires user-only terminal evidence or an owner decision, use `block --id ID --reason TEXT`, keep it visible in status, and continue only with independent P0 items. After P0 completion, process P1 items or record an owner-approved `DEFERRED` reason. Do not mark the Python phase ready while any P0 remains `PARTIAL`, `MISSING`, `BROKEN`, blocked, or without evidence, or any P1 remains unclassified.

  **Expected:** `uv run python scripts/niki_workflow.py readiness --phase python` returns exit code `0` only when every Python P0 row is `WORKS` with evidence and the owner decision packet is resolved.

---

### Task 5: Complete bundled owner verification and the actual Niki CLI smoke

**Files:**
- Modify: `docs/niki/OWNER_VERIFY.md` with observed results
- Modify: `docs/niki/CHECKLIST.md` and `docs/niki/PROGRESS.md` with evidence-backed statuses
- Modify: `docs/niki/evaluations/SMOKE.md` with actual CLI transcript or `NOT RUN`

**Interfaces:**
- The owner verification packet is the current `docs/niki/OWNER_VERIFY.md`; do not convert its interactions into synthetic Pilot tests.
- The process smoke runs the `niki` console entry point with a model/provider configured to accept the measured Niki prompt context; Groq's observed 8,000-token free-tier limit is below the approximately 19,271-token system prompt and is not an eligible configuration for this smoke.

- [ ] **Step 1: Run all owner-only terminal checks in one session**

  Execute every item in `docs/niki/OWNER_VERIFY.md`: 16-color dark/light legibility, real `/theme` behavior, palette selection and Escape focus restoration, and completed/failed/cancelled summaries including approval pause/resume. Record terminal, size/color mode, exact actions, and observed results. Update each corresponding checklist row only after the owner supplies the observation.

- [ ] **Step 2: Run the process smoke through `niki -n`**

  In a fresh temporary Git repository, use the exact parser inputs in `docs/niki/EVALUATION.md`, start with the simple `partition(":")` implementation, and run the actual `niki -n` command using the configured model with sufficient context. Require all parser tests to pass, require `parser.py` to be the only modified file, and record command, model ID, exit code, test output, source commit, and diff summary in `SMOKE.md`. If provider access still fails, record the exact provider error and leave CLI acceptance blocked; do not substitute the SDK smoke.

- [ ] **Step 3: Re-run Python readiness checks**

  Run `cd libs/code && make check` and `uv run python scripts/niki_workflow.py readiness --phase python`, plus the real-terminal checks listed in `docs/niki/OWNER_VERIFY.md`. Expected: exit `0`, zero unresolved P0 rows, no secret values in artifacts, and an empty `libs/deepagents` diff. Only then move workflow state to `python_evaluation`.

---

### Task 6: Run the final matched Python evaluation and open the Rust gate

**Files:**
- Modify: `docs/niki/evaluations/SMOKE.md`
- Modify: `docs/niki/evaluations/BASELINE.md`
- Create: `docs/niki/evaluations/terminal-bench-baseline-comparison.json`
- Modify: `docs/niki/RUST_MIGRATION_GATE.md`

**Interfaces:**
- The comparator consumes complete Harbor 0.25.0 job directories and requires matching tasks, checksums, and exactly three attempts per task; no partial report is valid.
- The final report records the product commit, harness commit, both job IDs, exact protocol, every task result, resolution rates, paired 95% interval, pass@1/pass@3, durations, tokens, and cost where Harbor reports it.
- The baseline phase is `first_post_task_11`; do not create the comparison JSON from incomplete jobs.

- [ ] **Step 1: Check the final evaluation environment without exposing credentials**

  Confirm a working supported Harbor sandbox runtime, `ANTHROPIC_API_KEY`, `LANGSMITH_API_KEY`, a published branch, and workflow/runner access. The last local check found Docker unavailable, Podman unable to initialize because its runtime path is read-only, and Anthropic/LangSmith/Groq keys unset; recheck because this can change. Record only set/unset and runtime status. If any required part is unavailable, leave the final comparison blocked and record the exact reason; no local fallback can produce a valid matched score.

- [ ] **Step 2: Run the paired two-task smoke only after Python readiness passes**

  Run both Make targets with `MAX_TASKS=2`:

  ```bash
  make -C libs/evals run-niki-terminal-bench MAX_TASKS=2
  make -C libs/evals run-claude-code-terminal-bench MAX_TASKS=2
  ```

  Verify each config matches the fixed protocol, both jobs contain the same two tasks and checksums, each task has three attempts per agent, retries are zero, and costs are recorded from observed job metadata. Add the job IDs and effective configuration to `SMOKE.md`. Do not continue if either job is incomplete or settings differ.

- [ ] **Step 3: Run the full matched benchmark and unified regression eval**

  Run both Terminal-Bench jobs without `MAX_TASKS` on the complete pinned dataset:

  ```bash
  make -C libs/evals run-niki-terminal-bench
  make -C libs/evals run-claude-code-terminal-bench
  ```

  Run the unified `autonomous`/`dcode` `lite` profile with three rollouts against the exact final Python commit, using the command from Task 3, and compare it with the measured current Python completion baseline. Preserve all failures and timeouts as failures. Do not filter tasks, replace trials, or merge mismatched revisions.

- [ ] **Step 4: Generate and review the evidence artifacts**

  From `libs/evals`, run the comparator with both job directories, the exact product and harness commits, and `--baseline-phase first_post_task_11`:

  ```bash
  uv run python scripts/compare_niki_terminal_bench.py \
    harbor-jobs/niki-terminal-bench/<job-id> \
    harbor-jobs/claude-code-terminal-bench/<job-id> \
    --niki-agent langgraph \
    --out ../../docs/niki/evaluations/terminal-bench-baseline-comparison.json \
    --niki-commit <product-commit> \
    --harness-commit <evaluation-commit> \
    --baseline-phase first_post_task_11
  ```

  Complete `docs/niki/evaluations/BASELINE.md` from observed data. Claim competitiveness only if Niki resolution rate is at least 55% and the paired 95% interval lower bound is at least -10 percentage points; otherwise publish the result as a measurement without the claim. Open the Rust gate only if the matched benchmark passes, unified avg@3 is no more than 5 percentage points below baseline, all deterministic safety/failure-injection gates pass, and every earlier Python gate remains green.

- [ ] **Step 5: Update workflow state and hand off to Rust only if the gate opens**

  Set `workflow-state.json` to phase `rust` only after all criteria in `RUST_MIGRATION_GATE.md` pass. Otherwise keep the phase `python_evaluation`, add task cards from reproducible failed-task evidence, and stop before Rust. The evaluation report must explicitly state the result and the next queued item.

---

### Task 7: Continue the same dynamic workflow through Rust parity

**Files:**
- Create: `libs/niki-rs/Cargo.toml`, `libs/niki-rs/src/main.rs`, and modules under `src/domain/`, `src/application/`, `src/adapters/runtime/`, and `src/adapters/terminal/` after Task 6 opens the gate
- Create: `libs/evals/niki_rust_harbor/agent.py` implementing Harbor 0.25.0's local `BaseAgent` contract
- Modify: `libs/evals/Makefile` with `run-niki-rust-terminal-bench`
- Modify: `libs/evals/scripts/compare_niki_terminal_bench.py` to report the selected Niki agent identity
- Create: `docs/niki/evaluations/terminal-bench-rust-comparison.json`
- Modify: Python adapters only as needed for the temporary runtime bridge
- Test: cross-language runtime-contract, Harbor-agent, and Niki acceptance tests
- Update: `docs/niki/CHECKLIST.md`, `docs/niki/PROGRESS.md`, `docs/niki/RUST_MIGRATION_GATE.md`

**Interfaces:**
- Preserve the framework-free domain/application contracts and versioned `RunEvent` JSON envelope as the language boundary.
- Keep Python as a temporary managed-runtime adapter in the first Rust phase. Rust owns the CLI/TUI and application flow one tested vertical slice at a time; do not duplicate persistence or graph execution while Python remains the runtime owner.
- Implement `NikiRustAgent.setup(self, environment: BaseEnvironment) -> None` and `NikiRustAgent.run(self, instruction: str, environment: BaseEnvironment, context: AgentContext) -> None` against Harbor 0.25.0's `BaseAgent` contract. Run the Rust `niki` command inside the task's isolated environment and populate Harbor context with its observed process result.
- Preserve the CLI contract `niki -M MODEL --model-params '{"reasoning_effort":"max"}' -S all -n PROMPT`; add the Harbor target with the same model, reasoning, LangSmith environment/plugin, timeout, attempt, retry, and task-set settings as Claude Code.
- The comparison protocol records `niki_agent=langgraph` for the Python gate and `niki_agent=niki-rust` for the Rust gate. The Rust run writes to `harbor-jobs/niki-rust-terminal-bench/` and is compared only with Claude Code jobs from the same dataset revision and protocol.
- Extend the comparator CLI with `--niki-agent niki-rust`; use `--baseline-phase rust_final` for the Rust artifact and preserve the same protocol field and thresholds for both agent implementations.
- The workflow skill and state file remain the task queue; Rust tasks are derived from the existing accepted Python behaviors and port/event contracts, so no new top-level plan is required.

- [ ] **Step 1: Create the Rust phase task cards from accepted contracts**

  Record one card per vertical slice: CLI startup/config, event protocol, terminal presentation, input/approval flow, session/history adapter, runtime bridge, tool/action presentation, and packaging. Each card names the Python behavior and tests it must preserve, a Rust module owner, its cross-language test, and its rollback boundary. Keep the Python status/evidence columns unchanged and update only the Rust columns as Rust cards pass. Do not start a slice that has no passing Python reference behavior.

- [ ] **Step 2: Port one vertical slice while Python remains the reference runtime**

  Implement the card's Rust module behind the existing port/event contract. Run Rust unit tests plus the matching Python/Rust contract fixture. Keep Niki's Python entry point usable until all P0 behavior has a Rust implementation and tests.

- [ ] **Step 3: Use the dynamic readiness loop for every Rust slice**

  For each slice, use the same select → test-first implement → review → evidence update → commit loop from Task 4. A behavior may move to `WORKS` for Rust only when its Python acceptance case has an equivalent passing Rust test and no UI, approval, or session behavior regresses. When all required Rust rows are `WORKS` with evidence, run `readiness --phase rust` and move to `rust_evaluation` only on exit code `0`.

- [ ] **Step 4: Run the final Rust verification and matched comparison**

  Add `test_niki_rust_harbor_agent_runs_cli_in_task_workspace` for command, workspace, model, and exit/result capture; add `test_rust_harbor_target_matches_claude_code_protocol` for the shared Harbor settings; and add `test_comparison_json_records_selected_niki_agent`. After all Python `WORKS` behaviors have Rust evidence, run `cargo test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, Python/Rust contract tests, Niki CLI/process smoke, security/failure-injection checks, and lint/type checks for remaining Python adapters. Run and validate a two-task Harbor smoke before the full benchmark:

  ```bash
  make -C libs/evals run-niki-rust-terminal-bench MAX_TASKS=2
  make -C libs/evals run-claude-code-terminal-bench MAX_TASKS=2
  ```

  Verify matching tasks/checksums and the fixed protocol, then run the complete Rust/Claude Code jobs and compare them:

  ```bash
  cd libs/evals
  uv run python scripts/compare_niki_terminal_bench.py \
    harbor-jobs/niki-rust-terminal-bench/<job-id> \
    harbor-jobs/claude-code-terminal-bench/<job-id> \
    --niki-agent niki-rust \
    --out ../../docs/niki/evaluations/terminal-bench-rust-comparison.json \
    --niki-commit <rust-product-commit> \
    --harness-commit <evaluation-commit> \
    --baseline-phase rust_final
  ```

  Switch the `niki` distribution entry point only after these checks pass. Preserve `dcode` and the Python bridge only where the migration contract still requires them.

- [ ] **Step 5: Mark the workflow complete with final evidence**

  Set `workflow-state.json` phase to `complete` only when every Python `WORKS` behavior has Rust evidence, all required Rust acceptance rows are `WORKS` or owner-approved `DEFERRED`, the Rust matched benchmark artifacts exist, the comparison passes both competitiveness thresholds, all required checks pass, and `libs/deepagents` remains untouched. If either threshold fails, keep phase `rust_evaluation`, publish the measurement without a competitiveness claim, and generate task cards from reproducible failures; do not mark the workflow complete unless the owner explicitly stops the effort. Commit the final readiness record only after the gate passes.

---

## Workflow status (2026-11-26)

The checkboxes above are left as written; this section records where the
workflow actually stands, so the plan and the product do not disagree.

| Task | Status | Evidence |
| --- | --- | --- |
| 1 — local evaluation-harness gaps | **Done** | `libs/evals` comparator tests and lint; recorded in `docs/niki/PROGRESS.md` Sessions 2–4. |
| 2 — dynamic completion workflow | **Done** | `libs/code/scripts/niki_workflow.py`, `docs/niki/workflow-state.json`, the task-card skill and its tests. |
| 3 — reconcile checklist, record a baseline | **Done** | `docs/niki/CHECKLIST.md` + `docs/niki/BASELINE.md`. |
| 4 — drive the Python P0 backlog | **Done** | `readiness --phase python` exits 0; 51 P0 rows resolved, 57 rows WORKS. |
| 5 — owner checks and the CLI smoke | **Run; smoke not passed** | Owner-only checks are in `docs/niki/OWNER_VERIFY.md`; the five local-model runs are `RUN, NOT PASSED` in `docs/niki/evaluations/SMOKE.md` and tracked in #7. |
| 6 — matched Python evaluation, open the Rust gate | **Blocked, not claimed** | `ANTHROPIC_API_KEY`/`LANGSMITH_API_KEY` unset and no usable container runtime (no Docker; Podman fails with `default OCI runtime "crun" not found`). Recorded in `docs/niki/evaluations/SMOKE.md`; tracked in #6. No SDK-level run is substituted. |
| 7 — Rust parity | **Not started** | `docs/niki/RUST_MIGRATION_GATE.md`: the gate requires a passed matched Python benchmark first. |

The Python product is shipped on `niki-agent` (`592e1876`, `7f1547aa`,
`71d49b65`). What remains is exactly what the gate says remains: the matched
comparison, then Rust — both waiting on the environment above, not on code.
