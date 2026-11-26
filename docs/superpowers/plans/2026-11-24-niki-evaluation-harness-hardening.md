# Niki Evaluation Harness Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Niki versus Claude Code evaluation reject mismatched evidence, prove both Harbor commands use the same benchmark protocol, and accurately record the process smoke.

**Architecture:** Keep the shared Harbor configuration and Make targets as the source of runner settings. Keep Harbor result parsing, validation, statistics, and JSON serialization in the eval package, with focused helpers and explicit paired-task invariants. This plan hardens the harness and smoke evidence; a later plan will run the credentialed Harbor smoke and full baseline.

**Tech Stack:** Python 3.12–3.13, Harbor 0.25.0, Terminal-Bench 4.1.0, uv, Make, pytest, and the existing Niki CLI.

**Spec:** `docs/superpowers/plans/2026-11-24-niki-benchmark-baseline-repair.md`; `libs/evals/AGENTS.md`; repository `AGENTS.md`.

## Global Constraints

- Use Harbor 0.25.0 in `libs/evals` and the official dataset ref `terminal-bench/terminal-bench@4.1.0`.
- Evaluate Niki with Harbor's `langgraph` agent and graph `dcode`; evaluate Claude Code with Harbor's `claude-code` agent.
- Match `anthropic:claude-opus-5-5`, reasoning effort `max`, LangSmith sandbox and plugin, timeout multiplier `1.0`, three attempts per task, one concurrent trial, and zero Harbor retries.
- `MAX_TASKS=2` limits the smoke task set only; it must not change three attempts per task. An unset `MAX_TASKS` means the complete pinned task set.
- Keep Niki and Claude Code job artifacts separate. Never make a comparison from missing attempts, unequal task sets, or different task revisions.
- Label any new capture `first post-Task-11 baseline`; the missed pre-Task-1 baseline cannot be reconstructed.
- Claim “competitive with Claude Code” only if Niki's mean trial resolution rate is at least 55% and the lower bound of the paired 95% confidence interval is at least -10 percentage points.
- Keep `libs/deepagents` unchanged. Verify `git diff f57c6f383b7018024ca5cde2dc565048ea83202a -- libs/deepagents` is empty.
- Do not put provider or LangSmith secrets in source, test fixtures, logs committed to the repository, or evaluation artifacts.
- Pin GitHub Actions to full commit SHAs, as required by repository `AGENTS.md`.

## Review Focus

- A later trial for one task uses a different checksum than the first attempt. Pin this with `test_paired_comparison_rejects_revision_drift_within_agent` in Task 1.
- A malformed, missing, non-finite, or invalid Harbor field is accepted as evidence. Pin these cases with `test_load_trial_results_rejects_invalid_records` in Task 1.
- One runner silently changes dataset, model, effort, sandbox, attempts, retries, permissions, or output directory. Pin this with `test_harbor_runner_targets_use_same_terminal_bench_inputs` in Task 2.
- The added smoke workflow cannot be parsed or exposes credentials in its committed configuration. Pin YAML parsing and secret-reference checks with `test_niki_smoke_workflow_is_valid_and_secret_safe` in Task 2.
- An SDK-level test is reported as proof of the `niki -n` process path, or the process smoke changes unrelated files. Pin the CLI invocation, exit status, and file diff in the Task 3 smoke evidence.

## File Structure

- `libs/evals/scripts/compare_niki_terminal_bench.py` owns Harbor 0.25.0 trial loading, pair validation, statistics, protocol metadata, and JSON output.
- `libs/evals/tests/unit_tests/test_niki_competitor_eval.py` owns invalid-record, pairing, report, and runner parity tests.
- `libs/evals/tests/fixtures/harbor-0.25/terminal-bench-trial/result.json` remains the versioned Harbor result fixture consumed by loader tests.
- `libs/evals/Makefile` and `libs/evals/terminal-bench-4.1.yaml` own the matched job inputs.
- `.github/workflows/niki-smoke.yml` owns only an optional one-task CI smoke; it is not a matched baseline or comparison workflow.
- `docs/niki/EVALUATION.md` states the protocol; `docs/niki/evaluations/SMOKE.md` records observed process-smoke evidence and clearly labels any unrun CLI or Harbor checks.

---

### Task 1: Enforce strict paired-result validation

**Files:**
- Modify: `libs/evals/scripts/compare_niki_terminal_bench.py`
- Modify: `libs/evals/tests/unit_tests/test_niki_competitor_eval.py`
- Test: `libs/evals/tests/fixtures/harbor-0.25/terminal-bench-trial/result.json`

**Interfaces:**
- Preserve `load_trial_results(job_dir: Path) -> list[TrialResult]` and `compare_results(niki: Sequence[TrialResult], claude_code: Sequence[TrialResult], *, bootstrap_samples: int = 10_000, seed: int = 20261123) -> ComparisonReport`.
- Preserve `TrialResult`, `TaskOutcome`, `ComparisonReport`, and `ComparisonProtocol` field names and JSON shape defined in the prior plan.
- Add focused internal helpers `_read_trial_json(path: Path) -> dict[str, object]`, `_parse_trial_record(data: dict[str, object], source: Path) -> tuple[TrialResult, datetime, str]`, `_validate_paired_trials(niki_by_task: dict[str, list[TrialResult]], claude_code_by_task: dict[str, list[TrialResult]]) -> None`, and `_paired_bootstrap_ci(niki_by_task: dict[str, list[TrialResult]], claude_code_by_task: dict[str, list[TrialResult]], *, bootstrap_samples: int, seed: int) -> tuple[float, float]` so parsing and comparison validation remain independently understandable.

- [ ] **Step 1: Write the failing loader and comparison tests**

  Add `test_load_trial_results_reads_harbor_025_fixture` and parameterized `test_load_trial_results_rejects_invalid_records`. Cover malformed JSON; missing task name, checksum, trial ID, or timestamps; duplicate trial IDs or names; Boolean, NaN, infinite, and out-of-range rewards; negative duration, token counts, or cost; and a missing reward with and without execution `exception_info`. Any recorded execution failure must have reward `0.0` and status `failed` or `timed_out`, even if a malformed record also contains a nonzero reward.

  Add `test_paired_comparison_rejects_incomplete_or_mismatched_task_trials` cases for unequal task sets, a task absent from one agent, two or four attempts, and duplicate attempt indexes. Add `test_paired_comparison_rejects_revision_drift_within_agent` with Niki checksums `rev-A`, `rev-B`, `rev-A` and Claude Code checksums `rev-A`, `rev-A`, `rev-A`; expect `ValueError`. Also assert a checksum mismatch between agents is rejected.

  Add `test_report_uses_linear_interpolated_duration_percentiles` with durations `[0.0, 10.0, 20.0, 30.0]`; expect median `15.0` and p95 `28.5`. Add `test_comparison_json_contains_protocol_and_per_task_trials` and assert the protocol fields, job IDs, commits, baseline phase, all task IDs/checksums, all three outcomes per agent, resolution rate, pass@1, pass@3, paired CI, duration, tokens, and cost are serialized. Add `test_comparison_cli_requires_job_ids`: a job directory without a job-level ID must fail with `ValueError` rather than writing a report with an empty ID.

- [ ] **Step 2: Run the new comparator tests and confirm they fail**

  Run:

  ```bash
  cd libs/evals
  uv run pytest tests/unit_tests/test_niki_competitor_eval.py -q
  ```

  Expected: FAIL on the mixed-checksum case, missing validation cases, and any report assertions not yet implemented.

- [ ] **Step 3: Implement the validation and split the long functions**

  In `compare_niki_terminal_bench.py`, require all three attempts for a task to carry one checksum within each agent and require the Niki and Claude Code checksums to match. Reject duplicate/missing attempts before calculating statistics. Keep failure and timeout trials at reward `0.0`; do not convert missing trial files into results. Refactor `load_trial_results` and `compare_results` into the helper interfaces above and keep each helper focused in line with the repository guidance to keep functions about 20 lines or less.

- [ ] **Step 4: Run comparator and missing-rollout tests**

  Run:

  ```bash
  cd libs/evals
  uv run pytest tests/unit_tests/test_niki_competitor_eval.py -q
  cd ../..
  uv run --project libs/evals pytest .github/scripts/tests/evals/test_aggregate_shards.py::test_missing_rollouts_count_as_failures -q
  ```

  Expected: PASS; mixed revisions, malformed results, and incomplete comparisons cannot produce a report, while recorded failures remain zero-reward trials.

- [ ] **Step 5: Commit the comparator hardening**

  ```bash
  git add libs/evals/scripts/compare_niki_terminal_bench.py libs/evals/tests/unit_tests/test_niki_competitor_eval.py libs/evals/tests/fixtures/harbor-0.25/terminal-bench-trial/result.json
  git commit -m "fix(evals): reject invalid paired benchmark evidence"
  ```

---

### Task 2: Prove runner parity and repair the smoke workflow

**Files:**
- Modify: `libs/evals/tests/unit_tests/test_niki_competitor_eval.py`
- Modify: `libs/evals/Makefile` only if parity assertions expose a mismatch
- Modify: `libs/evals/terminal-bench-4.1.yaml` only if parity assertions expose a mismatch
- Modify: `.github/workflows/niki-smoke.yml`
- Test: `libs/evals/tests/unit_tests/test_harbor_langgraph_agent.py`

**Interfaces:**
- `run-niki-terminal-bench` uses Harbor `langgraph`, graph `dcode`; `run-claude-code-terminal-bench` uses Harbor `claude-code`.
- Both commands use `terminal-bench/terminal-bench@4.1.0`, Harbor model spelling `anthropic/claude-opus-5-5`, reasoning effort `max`, `langsmith` environment and plugin, timeout multiplier `1.0`, zero retries, and separate job directories.
- The shared YAML has `n_attempts: 3`, `n_concurrent_trials: 1`, `retry.max_retries: 0`, `agent_timeout_multiplier: 1.0`, and `environment.type: langsmith`.

- [ ] **Step 1: Replace the placeholder runner test with observable assertions**

  Replace `test_harbor_runner_targets_use_same_terminal_bench_inputs`'s unconditional `assert True`. Set `EVALS_DIR = Path(__file__).resolve().parents[2]`. For each target, call `subprocess.run(["make", "-n", target, *make_vars], check=True, capture_output=True, text=True, cwd=EVALS_DIR)` with `make_vars=[]` and `make_vars=["MAX_TASKS=2"]`; parse each rendered Harbor command with `shlex.split`.

  Assert the shared dataset, model, reasoning effort, environment, LangSmith plugin and dataset name, timeout, retries, three attempts, and distinct experiment names and job directories. Assert Niki selects `langgraph` with `graph=dcode` and Claude Code selects `claude-code` with `permission_mode=bypassPermissions`. Assert `MAX_TASKS=2` adds only `--n-tasks 2`, an unset value adds no `--n-tasks`, and neither command references Terminal-Bench 2. Parse `terminal-bench-4.1.yaml` and assert its exact attempt, concurrency, retry, timeout, and environment values.

- [ ] **Step 2: Run the runner parity test and confirm the current placeholder is replaced**

  Run:

  ```bash
  cd libs/evals
  uv run pytest tests/unit_tests/test_niki_competitor_eval.py::test_harbor_runner_targets_use_same_terminal_bench_inputs tests/unit_tests/test_harbor_langgraph_agent.py::test_make_graph_uses_benchmark_model_and_reasoning_effort -q
  ```

  Expected: PASS with both commands rendered and compared; the model adapter test observes `anthropic:claude-opus-5-5` and `reasoning_effort=max`.

- [ ] **Step 3: Make the optional GitHub smoke workflow valid and immutable**

  Remove the stray heredoc terminator from `.github/workflows/niki-smoke.yml`. Keep the job explicitly one-task and smoke-only; it uses Docker and its configured Groq model, so it is not evidence for the matched LangSmith/Opus comparison. Remove the duplicate `HARBOR_CONFIG` assignment. Pin `actions/checkout`, `astral-sh/setup-uv`, and `actions/upload-artifact` to full 40-character commit SHAs verified from their upstream release tags. Keep secrets referenced only through `${{ secrets.* }}` and do not print them.

- [ ] **Step 4: Add workflow validation and run the focused checks**

  Add `test_niki_smoke_workflow_is_valid_and_secret_safe`, parse the workflow with `yaml.BaseLoader` so the `on` key remains a string, assert it has a `workflow_dispatch` trigger and the expected smoke job, assert every `uses:` ref is a 40-character SHA, and assert no literal API-key value is present. Then run:

  ```bash
  cd libs/evals
  uv run pytest tests/unit_tests/test_niki_competitor_eval.py tests/unit_tests/test_harbor_langgraph_agent.py tests/unit_tests/test_harbor_langsmith_integration.py -q
  uv lock --check
  uv run harbor --version
  ```

  Expected: PASS; Harbor reports `0.25.0`, the lock is current, and both runner and adapter contracts are covered by assertions.

- [ ] **Step 5: Commit runner and workflow validation**

  ```bash
  git add libs/evals/tests/unit_tests/test_niki_competitor_eval.py .github/workflows/niki-smoke.yml
  git add libs/evals/Makefile libs/evals/terminal-bench-4.1.yaml libs/evals/tests/unit_tests/test_harbor_langgraph_agent.py
  git commit -m "test(evals): verify matched Harbor runner settings"
  ```

---

### Task 3: Complete protocol documentation and verify the Niki CLI smoke

**Files:**
- Modify: `docs/niki/EVALUATION.md`
- Modify: `docs/niki/evaluations/SMOKE.md`
- Test: temporary parser-smoke repository outside the checkout

**Interfaces:**
- The process smoke prompt requires `parse_host_port` to accept `example.test:443`, `127.0.0.1:80`, and `[2001:db8::1]:443`; reject `[2001:db8::1:443`, `2001:db8::1]:443`, `host:abc`, and `host:65536`; then pass `python -m pytest -q test_parser.py`.
- The process run uses the actual `niki` console entry point, model `groq:qwen/qwen3.8-27b`, non-interactive mode `-n`, and temporary repository as its working directory. Give the agent shell access only inside this disposable temp repository.

- [ ] **Step 1: Document the exact evaluation protocol**

  Update `docs/niki/EVALUATION.md` to include the exact dataset ref, Harbor version, both agents, graph `dcode`, model and effort, LangSmith sandbox/plugin, timeout, concurrency, attempts, retries, separate outputs, and first-post-Task-11 label. Define trial-weighted resolution rate, pass@1, pass@3, paired bootstrap with 10,000 samples and seed `20261123`, linear-interpolated duration percentiles, token/cost reporting, and the two competitiveness thresholds. State that the planned pre-Task-1 baseline was missed and cannot be reconstructed.

- [ ] **Step 2: Run the parser case through `niki -n` in a fresh temporary repo**

  Create `parser.py` with the simple `partition(":")` implementation and `test_parser.py` with the two named tests and exact inputs in Interfaces. Initialize a Git repository and commit the starting files. From the temp repository, run:

  ```bash
  uv run --project <checkout>/libs/code niki \
    -M groq:qwen/qwen3.8-27b -S all -n \
    "Update only parser.py so parse_host_port returns (host, integer_port) for example.test:443, 127.0.0.1:80, and [2001:db8::1]:443; raise ValueError for [2001:db8::1:443, 2001:db8::1]:443, host:abc, and host:65536. Run python -m pytest -q test_parser.py and continue until it passes."
  ```

  Record stdout, stderr, exit code, the final `python -m pytest -q test_parser.py` output, Niki's tested source commit, and `git diff -- parser.py`. Require `parser.py` to be the only modified file and require both parser tests to pass. If the required Groq credential or provider access is unavailable, record the CLI smoke as `NOT RUN` and retain the earlier SDK smoke only as SDK-level evidence; do not describe it as CLI validation.

- [ ] **Step 3: Correct and complete the smoke evidence**

  Remove the stray literal `EOF` from `docs/niki/evaluations/SMOKE.md`. Label the previous Deep Agents SDK run as SDK-level. Add the actual CLI run transcript and file-diff summary only if Step 2 ran successfully; otherwise record the exact missing credential or service and `NOT RUN`. Keep the matched Harbor smoke section `NOT RUN` until a future credentialed-run plan executes it.

- [ ] **Step 4: Review docs and smoke evidence, then commit**

  Verify the protocol values agree with `libs/evals/Makefile`, `libs/evals/terminal-bench-4.1.yaml`, and the comparator's protocol fields. Run `git diff --check`, then:

  ```bash
  git add docs/niki/EVALUATION.md docs/niki/evaluations/SMOKE.md
  git commit -m "docs(evals): clarify Niki smoke evidence"
  ```

---

### Task 4: Verify the hardened harness and prepare the live-run handoff

**Files:**
- Verify: `libs/evals/scripts/compare_niki_terminal_bench.py`
- Verify: `libs/evals/Makefile`
- Verify: `libs/evals/terminal-bench-4.1.yaml`
- Verify: `.github/workflows/niki-smoke.yml`
- Verify: `docs/niki/EVALUATION.md`, `docs/niki/evaluations/SMOKE.md`

**Interfaces:**
- No new runtime interface. This task verifies the unchanged comparator/runner contract and records the remaining live-run prerequisites for the following execution plan.

- [ ] **Step 1: Run all focused local checks**

  Run:

  ```bash
  cd libs/evals
  uv lock --check
  make lint
  uv run pytest tests/unit_tests/test_niki_competitor_eval.py tests/unit_tests/test_harbor_langgraph_agent.py tests/unit_tests/test_harbor_langsmith_integration.py -q
  uv run harbor --version
  make -n run-niki-terminal-bench MAX_TASKS=2
  make -n run-claude-code-terminal-bench MAX_TASKS=2
  cd ../..
  git diff --check
  git diff f57c6f383b7018024ca5cde2dc565048ea83202a -- libs/deepagents
  ```

  Expected: all checks pass, Harbor reports `0.25.0`, both dry runs show the same matched inputs and two-task cap, and the `libs/deepagents` diff is empty.

- [ ] **Step 2: Record live-run prerequisites without fabricating results**

  Record in the handoff report whether the execution host has a working Docker or supported Harbor sandbox runtime, `ANTHROPIC_API_KEY`, `LANGSMITH_API_KEY`, and a usable GitHub Actions dispatch context. Report only set/unset status, never values. Do not create `terminal-bench-baseline-comparison.json` or edit `BASELINE.md` with scores in this plan; those artifacts require successful matched jobs and belong to the next live-evaluation plan.

- [ ] **Step 3: Commit any final verification-only documentation adjustment**

  If Task 3 already records all remaining live-run prerequisites, make no extra file change. Otherwise add a short “Next run prerequisites” note to `SMOKE.md` listing only the unavailable services/credentials, then commit it with `git commit -m "docs(evals): record live-run prerequisites"`.

---
