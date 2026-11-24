# Niki Matched Coding Evaluation Baseline Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Make Niki’s coding-quality comparison reproducible, repair the Harbor runner and result-validation gaps, and capture the first trustworthy matched baseline for the current Niki implementation.

**Architecture:** Keep evaluation code and its pinned Harbor dependency inside libs/evals. Both compared agents use one Terminal-Bench job configuration and one result schema; only the agent adapter and output directory differ. The pre-Task-1 baseline was not captured, so the new run is labeled as the first post-Task-11 baseline rather than being presented as historical evidence.

**Tech Stack:** Python >=3.12,<3.14, Harbor 0.25.0, Terminal-Bench 4.1.0, uv, Make, pytest, and the existing LangGraph dcode Harbor adapter.

**Spec:** Task 0 in docs/superpowers/plans/2026-11-23-niki-agent-architecture.md; docs/niki/EVALUATION.md; libs/evals/AGENTS.md; libs/evals/UNIFIED_EVALS.md.

## Global Constraints

- Use Harbor 0.25.0 in both libs/evals/pyproject.toml and libs/evals/uv.lock. Keep Harbor and benchmark dependencies out of libs/code runtime dependencies.
- Use the official dataset ref terminal-bench/terminal-bench@4.1.0. Recheck the official release before running; if 4.1.0 is retired, pin the then-current official version and collect a new matched Claude Code baseline. Never compare scores across benchmark versions.
- Evaluate Niki through Harbor’s langgraph agent with graph dcode. Evaluate the competitor through Harbor’s claude-code agent. Do not substitute graph bare for the Niki result.
- Match model anthropic:claude-opus-5-5, reasoning effort max, LangSmith sandbox and plugin, agent timeout multiplier 1.0, three attempts per task, zero Harbor retries, and unattended execution inside Harbor's isolated task sandbox. Use the same task set and sandbox settings for both agents.
- A two-task smoke run uses MAX_TASKS=2 and still runs three attempts per task. With MAX_TASKS unset, run the complete pinned dataset. Harbor concurrency is not the number of attempts.
- Do not modify or filter the pinned task set after a comparison begins. Keep Niki and Claude Code job artifacts in separate directories.
- A comparison is “competitive with Claude Code” only when Niki’s mean trial resolution rate is at least 55% and the lower bound of the paired 95% confidence interval is at least -10 percentage points. Otherwise publish the measurements without that claim.
- The planned pre-change baseline was missed. Label this capture “first post-Task-11 baseline”; do not invent or infer pre-change results.
- Keep libs/deepagents unchanged. Before commit, verify git diff f57c6f383b7018024ca5cde2dc565048ea83202a -- libs/deepagents is empty.
- Official references at plan time: [Terminal-Bench v4.1.0 release](https://github.com/harbor-framework/terminal-bench/releases/tag/v4.1.0) and [Harbor v0.25.0 release](https://github.com/harbor-framework/harbor/releases/tag/v0.25.0).
- Harbor 0.25.0's per-trial schema and path are defined in its versioned [TrialResult model](https://github.com/harbor-framework/harbor/blob/v0.25.0/src/harbor/models/trial/result.py) and [TrialPaths model](https://github.com/harbor-framework/harbor/blob/v0.25.0/src/harbor/models/trial/paths.py); use these rather than a newer Harbor schema.

## Review Focus

- A target accidentally runs Terminal-Bench 2, changes the Harbor version, or points Niki at graph bare. Pin this with test_harbor_runner_targets_use_same_terminal_bench_inputs.
- A target confuses concurrency with attempts or silently enables retries. Assert n_attempts is 3, n_concurrent_trials is fixed independently, and retry.max_retries is 0 in test_harbor_runner_targets_use_same_terminal_bench_inputs.
- Harbor’s LangGraph adapter receives a different model or reasoning setting than the documented protocol. Pin the constructed model and reasoning_effort in test_make_graph_uses_benchmark_model_and_reasoning_effort.
- Missing, duplicate, malformed, non-finite, or different-task-revision trial records silently improve a score. Pin rejection and failure behavior in test_load_trial_results_rejects_invalid_records and test_paired_comparison_rejects_incomplete_or_mismatched_task_trials.
- A current run is mislabeled as pre-change evidence or omits task-level outcomes. Pin the baseline phase and full JSON shape in test_comparison_json_contains_protocol_and_per_task_trials.

## File Structure

- libs/evals/terminal-bench-4.1.yaml owns shared attempt count, retry, agent timeout, concurrency, and environment settings for both Harbor jobs.
- libs/evals/Makefile owns the two Harbor commands, agent selection, model and reasoning wiring, LangSmith plugin arguments, smoke task limit, and separate output directories.
- libs/evals/deepagents_harbor/langgraph_project/langgraph_agent.py maps the Niki benchmark model and reasoning setting into the existing model construction path.
- libs/evals/scripts/compare_niki_terminal_bench.py owns Harbor result loading, paired validation, statistics, and machine-readable report output.
- libs/evals/tests/fixtures/harbor-0.25/terminal-bench-trial/result.json pins the exact result shape consumed by the comparator tests.
- libs/evals/tests/unit_tests/test_niki_competitor_eval.py owns runner parity and comparison/report behavior tests; test_harbor_langgraph_agent.py owns model-construction coverage; test_harbor_langsmith_integration.py owns the existing Harbor dependency and plugin contract.
- docs/niki/EVALUATION.md and docs/niki/evaluations/ own the protocol, smoke evidence, and baseline. libs/evals/UNIFIED_EVALS.md must agree with the Harbor version used by the locked environment.

---

### Task 1: Pin and prove matched Harbor jobs

**Files:**
- Create: libs/evals/terminal-bench-4.1.yaml
- Modify: libs/evals/pyproject.toml
- Modify: libs/evals/uv.lock
- Modify: libs/evals/Makefile
- Modify: libs/evals/deepagents_harbor/langgraph_project/langgraph_agent.py
- Modify: libs/evals/tests/unit_tests/test_niki_competitor_eval.py
- Modify: libs/evals/tests/unit_tests/test_harbor_langgraph_agent.py
- Modify: libs/evals/tests/unit_tests/test_harbor_langsmith_integration.py
- Modify: libs/evals/UNIFIED_EVALS.md

**Interfaces:**
- The shared Harbor YAML sets n_attempts: 3, agent_timeout_multiplier: 1.0, n_concurrent_trials: 1, retry.max_retries: 0, and environment.type: langsmith.
- run-niki-terminal-bench invokes Harbor’s langgraph agent with project_path=deepagents_harbor/langgraph_project, config=langgraph.json, graph=dcode. run-claude-code-terminal-bench invokes claude-code. Both use the shared YAML, dataset ref terminal-bench/terminal-bench@4.1.0, --env langsmith, --plugin langsmith, --plugin-kwarg dataset_name=terminal-bench/terminal-bench@4.1.0, and a distinct --plugin-kwarg experiment_name.
- Both targets pass the same semantic model anthropic:claude-opus-5-5 and reasoning_effort=max. Define NIKI_BENCH_MODEL=anthropic:claude-opus-5-5, NIKI_BENCH_REASONING_EFFORT=max, HARBOR_SANDBOX_ENV=langsmith, HARBOR_AGENT_TIMEOUT_MULTIPLIER=1.0, and HARBOR_RETRIES=0 as overridable Make defaults. Convert the model to Harbor’s anthropic/claude-opus-5-5 spelling for --model; the LangGraph adapter converts it back to the colon form required by init_chat_model. Pass reasoning_effort=max to the LangGraph model via configurable.model_kwargs and to Claude Code via --agent-kwarg reasoning_effort=max. Set Claude Code permission_mode=bypassPermissions, which Harbor's 0.25.0 ClaudeCodeOptions uses by default, and keep Niki's dcode graph in headless mode inside the Harbor sandbox. The parity test asserts provider, model, reasoning effort, dataset, environment, plugin, timeout, attempts, retries, and permissions match. Map the sandbox, timeout, and retry Make variables to Harbor’s --env, --agent-timeout-multiplier, and --max-retries options and verify the resolved config.json has the fixed values.
- MAX_TASKS=2 adds a two-task limit; an unset MAX_TASKS runs the full dataset. The limit must not change the three attempts per task. Jobs write to harbor-jobs/niki-terminal-bench/ and harbor-jobs/claude-code-terminal-bench/.

- [ ] **Step 1: Write the failing runner and model-wiring tests**

  Add test_harbor_runner_targets_use_same_terminal_bench_inputs. Render both Make targets with MAX_TASKS=2 and without it. Assert the same dataset, provider/model identity, effort, LangSmith environment and plugin, timeout, three attempts, zero retries, Claude Code permission_mode=bypassPermissions, Niki graph=dcode running headless, and separate output directories. Assert Niki uses langgraph, Claude Code uses claude-code, only smoke commands contain the two-task limit, and concurrency is not mistaken for attempts. Assert neither command mentions terminal-bench-2.

  Add test_make_graph_uses_benchmark_model_and_reasoning_effort to test_harbor_langgraph_agent.py. Pass Harbor's anthropic/claude-opus-5-5 model and model_kwargs containing reasoning_effort=max through the graph config; patch init_chat_model and assert it receives anthropic:claude-opus-5-5 and reasoning_effort=max.

- [ ] **Step 2: Run the new tests to confirm the current gaps**

  Run:
  ~~~bash
  cd libs/evals
  uv run pytest tests/unit_tests/test_niki_competitor_eval.py::test_harbor_runner_targets_use_same_terminal_bench_inputs tests/unit_tests/test_harbor_langgraph_agent.py::test_make_graph_uses_benchmark_model_and_reasoning_effort -q
  ~~~

  Expected: FAIL because the targets still use terminal-bench-2 and the parity test currently ends with an unconditional pass.

- [ ] **Step 3: Implement the shared Harbor configuration and targets**

  Add the shared YAML values listed in Interfaces. Update both Make targets to use Harbor’s --config, --dataset, --n-tasks, --agent, --model, --agent-kwarg, --agent-env, --env, --plugin, --plugin-kwarg, --agent-timeout-multiplier, --max-retries, and --jobs-dir interfaces. Set MAX_TASKS through --n-tasks only when supplied. Preserve the LangSmith plugin’s dataset and distinct experiment names in both commands. Keep the LangGraph graph and Claude Code model settings semantically identical; add the smallest adapter mapping needed to normalize the selected Harbor model and pass reasoning effort into the LangGraph config. Forward the existing provider and LangSmith credentials through --agent-env; do not put secrets in the YAML or result artifacts.

  Set the Harbor dependency to exactly 0.25.0, regenerate uv.lock with uv lock, and update UNIFIED_EVALS.md so its pinned-version statement matches the locked runtime.

- [ ] **Step 4: Run focused tests and verify the dependency lock**

  Run:
  ~~~bash
  cd libs/evals
  uv lock --check
  uv run pytest tests/unit_tests/test_niki_competitor_eval.py tests/unit_tests/test_harbor_langgraph_agent.py tests/unit_tests/test_harbor_langsmith_integration.py -q
  uv run harbor --version
  ~~~

  Expected: PASS; Harbor reports 0.25.0, both dry-rendered jobs have identical benchmark inputs, and the model adapter test observes the pinned model and max effort.

- [ ] **Step 5: Commit the runner configuration**

  ~~~bash
  git add libs/evals/terminal-bench-4.1.yaml libs/evals/pyproject.toml libs/evals/uv.lock libs/evals/Makefile libs/evals/deepagents_harbor/langgraph_project/langgraph_agent.py libs/evals/tests/unit_tests/test_niki_competitor_eval.py libs/evals/tests/unit_tests/test_harbor_langgraph_agent.py libs/evals/tests/unit_tests/test_harbor_langsmith_integration.py libs/evals/UNIFIED_EVALS.md
  git commit -m "chore(evals): pin matched Terminal-Bench runner"
  ~~~

---

### Task 2: Reject incomplete comparisons and preserve per-task evidence

**Files:**
- Create: libs/evals/tests/fixtures/harbor-0.25/terminal-bench-trial/result.json
- Modify: libs/evals/scripts/compare_niki_terminal_bench.py
- Modify: libs/evals/tests/unit_tests/test_niki_competitor_eval.py

**Interfaces:**
- TrialResult contains task_id: str, task_checksum: str, trial_id: str, attempt_index: int, status: Literal["completed", "failed", "timed_out"], reward: float, duration_seconds: float, total_tokens: int | None, and cost_usd: float | None.
- TaskOutcome contains task_id: str, niki_trials: tuple[TrialResult, ...], and claude_code_trials: tuple[TrialResult, ...].
- ComparisonReport retains the current aggregate fields and adds task_outcomes: tuple[TaskOutcome, ...]. ComparisonProtocol contains benchmark_ref: str, harbor_version: str, niki_agent: str, claude_code_agent: str, model: str, reasoning_effort: str, sandbox: str, plugin: str, permission_mode: str, agent_timeout_multiplier: float, n_concurrent_trials: int, retries: int, trials_per_task: int, niki_job_id: str, claude_code_job_id: str, niki_commit: str, harness_commit: str, and baseline_phase: Literal["first_post_task_11"].
- write_comparison_json(report: ComparisonReport, protocol: ComparisonProtocol, *, output_path: Path) -> None writes the protocol and complete report. The CLI adds --niki-commit, --harness-commit, and --baseline-phase first_post_task_11 alongside the existing --out option, and reads both job IDs from their Harbor job-level result.json files.
- load_trial_results(job_dir: Path) -> list[TrialResult] reads one Harbor 0.25.0 trial result from each trials/*/result.json. It maps task_name, id, task_checksum, verifier_result.rewards.reward, exception_info, started_at/finished_at, and agent_result token/cost fields; it rejects unreadable JSON, duplicate trial IDs or trial names, missing timestamps, invalid task IDs/checksums, non-finite or out-of-range rewards, negative durations/tokens/costs, and absent required fields. A missing reward is accepted only when exception_info records an execution failure, then becomes 0.0. Attempt indexes are assigned per task by ascending started_at, with trial_name as a deterministic tie-breaker.
- compare_results(niki: Sequence[TrialResult], claude_code: Sequence[TrialResult], *, bootstrap_samples: int = 10_000, seed: int = 20261123) -> ComparisonReport requires a non-empty identical task set, matching task_checksum per task, and exactly three attempts per task for both agents. Attempt indexes are 1, 2, and 3. An explicit timeout/error trial with no reward is a failure with reward 0.0; an absent expected trial makes the comparison incomplete and raises ValueError.
- Resolution rate is the mean across all trials. pass@1 is the fraction of tasks whose first attempt has reward 1.0; pass@3 is the fraction with any successful attempt. The paired bootstrap resamples task IDs with replacement and computes the 2.5th and 97.5th percentiles from 10,000 samples using linear interpolation. Median and p95 durations use the same documented percentile rule.
- The JSON report includes protocol metadata, every task ID, all three Niki and Claude Code trial outcomes per task, aggregate scores, confidence interval, duration, token totals, and cost where Harbor provides it.

- [ ] **Step 1: Add failure and report-shape tests**

  Add a minimal Harbor 0.25.0 fixture at tests/fixtures/harbor-0.25/terminal-bench-trial/result.json using the fields in Interfaces, and update _write_trial to create that result.json schema with a unique trial id and ordered timestamps. Add test_load_trial_results_rejects_invalid_records for malformed JSON, duplicate task/attempt, NaN and Boolean rewards, rewards outside [0, 1], negative duration, and missing task identity. A missing reward is accepted only when exception_info records a failed attempt, which is normalized to reward 0.0. Extend test_paired_comparison_rejects_incomplete_or_mismatched_task_trials to assert that missing tasks, unequal task sets, mismatched task checksums, duplicate attempts, and fewer or more than three attempts raise ValueError.

  Extend test_paired_bootstrap_reports_zero_for_identical_agents with three attempt indexes per task and assert identical agents have a zero difference and CI (0.0, 0.0). Add test_report_uses_linear_interpolated_duration_percentiles using [0.0, 10.0, 20.0, 30.0] and asserting median 15.0 and p95 28.5; assert bootstrap_samples=0 raises ValueError. Add test_comparison_json_contains_protocol_and_per_task_trials and assert the artifact contains the fixed protocol values, first_post_task_11 label, both job IDs, Niki commit, each task ID/checksum, all per-attempt rewards, pass@1, pass@3, paired CI, and aggregate duration/token/cost fields.

- [ ] **Step 2: Run the new tests to verify they fail**

  Run:
  ~~~bash
  cd libs/evals
  uv run pytest tests/unit_tests/test_niki_competitor_eval.py -q
  ~~~

  Expected: FAIL because current records have no validated attempt indexes, incomplete trial counts are accepted, and reports omit per-task outcomes and protocol evidence.

- [ ] **Step 3: Implement strict Harbor result loading and comparison**

  Implement the TrialResult, TaskOutcome, ComparisonProtocol, and write_comparison_json interfaces above. Harbor 0.25.0 stores the single-trial file as trials/<trial-name>/result.json; task_name and task_checksum identify the paired task, id identifies the unique trial, verifier_result.rewards.reward is the score, exception_info identifies a run failure, and started_at/finished_at give the wall-clock duration. Sum n_input_tokens and n_output_tokens and cost_usd from agent_result; use null when Harbor did not report usage. Normalize recorded timeout/error attempts to status timed_out/failed and reward 0.0; reject an absent expected attempt. Sort attempts by started_at, then trial_name, and serialize all per-task results alongside the aggregate report. Keep the bootstrap deterministic with the fixed seed and no additional runtime dependency.

- [ ] **Step 4: Run evaluator tests and the existing missing-rollout guard**

  Run:
  ~~~bash
  cd libs/evals
  uv run pytest tests/unit_tests/test_niki_competitor_eval.py -q
  cd ../..
  uv run --project libs/evals pytest .github/scripts/tests/evals/test_aggregate_shards.py::test_missing_rollouts_count_as_failures -q
  ~~~

  Expected: PASS; incomplete paired jobs cannot produce a passing report, explicit failed attempts remain failures, and identical fixtures produce zero paired differences.

- [ ] **Step 5: Commit the validated comparator**

  ~~~bash
  git add libs/evals/scripts/compare_niki_terminal_bench.py libs/evals/tests/unit_tests/test_niki_competitor_eval.py
  git commit -m "fix(evals): validate paired coding benchmark results"
  ~~~

---

### Task 3: Document the protocol and validate live smoke runs

**Files:**
- Modify: docs/niki/EVALUATION.md
- Create: docs/niki/evaluations/SMOKE.md
- Modify: libs/evals/UNIFIED_EVALS.md

**Interfaces:**
- The fixed process smoke case uses a fresh temporary repository. Niki must update only parser.py so parse_host_port accepts example.test:443, 127.0.0.1:80, and [2001:db8::1]:443; it must raise ValueError for [2001:db8::1:443, 2001:db8::1]:443, host:abc, and host:65536; then python -m pytest -q test_parser.py must pass.
- test_parser.py contains test_parse_host_port_accepts_hosts_and_bracketed_ipv6 asserting the exact hostname/IPv4/IPv6 return pairs and test_parse_host_port_rejects_malformed_or_out_of_range_ports asserting ValueError for the four invalid inputs. Start parser.py with the current simple partition(':') implementation so the IPv6 case fails before Niki edits it.
- docs/niki/EVALUATION.md documents the exact benchmark ref, Harbor version, agents, model and effort, sandbox/plugin, timeout, attempts, retry behavior, metric definitions, bootstrap seed/method, cost reporting, and gate thresholds.
- docs/niki/evaluations/SMOKE.md records the parser-smoke transcript and diff summary plus both Harbor smoke job IDs, effective config values, task IDs/checksums, attempt counts, and observed spend. It contains no API keys or estimated scores.

- [ ] **Step 1: Complete the evaluation protocol and parser smoke specification**

  Update docs/niki/EVALUATION.md with the Interface values and the exact valid/invalid parser inputs. State that the missing pre-Task-1 results cannot be reconstructed and that this work is preparing a first post-Task-11 capture.

- [ ] **Step 2: Run the evaluator and Harbor integration checks**

  Run:
  ~~~bash
  cd libs/evals
  uv run pytest tests/unit_tests/test_niki_competitor_eval.py tests/unit_tests/test_harbor_langgraph_agent.py tests/unit_tests/test_harbor_langsmith_integration.py -q
  uv lock --check
  uv run harbor --version
  ~~~

  Expected: PASS; the locked Harbor version is 0.25.0 and the tests prove the shared command inputs and LangSmith plugin wiring.

- [ ] **Step 3: Run the process-level Niki smoke case**

  In a new temporary repository, run the actual niki -n entry point with this prompt: "Update only parser.py so parse_host_port returns (host, integer_port) for example.test:443, 127.0.0.1:80, and [2001:db8::1]:443; raise ValueError for [2001:db8::1:443, 2001:db8::1]:443, host:abc, and host:65536. Run python -m pytest -q test_parser.py and continue until it passes." Save stdout, stderr, exit code, verifier output, Niki commit, and git diff -- parser.py; require parser.py to be the only modified file and the pytest command to pass. Add observed evidence to SMOKE.md.

- [ ] **Step 4: Run both matched two-task Harbor smoke jobs**

  From libs/evals, run:
  ~~~bash
  make run-niki-terminal-bench MAX_TASKS=2
  make run-claude-code-terminal-bench MAX_TASKS=2
  ~~~

  Read each job's config.json and per-trial result.json files. Require the same two task names and task checksums, exactly three attempts per task, matching model/effort/environment/plugin/timeout/permissions, and zero retries. Record both job IDs, effective settings, result counts, and actual spend in SMOKE.md. Do not start the full benchmark unless both smoke jobs satisfy these checks.

- [ ] **Step 5: Update the unified-evaluation Harbor version reference**

  Update libs/evals/UNIFIED_EVALS.md anywhere it claims the locked Harbor version is 0.20.0. Keep its statements about Harbor behavior only if they still pass the existing plugin integration tests under 0.25.0.

- [ ] **Step 6: Commit the verified protocol and smoke evidence**

  ~~~bash
  git add docs/niki/EVALUATION.md docs/niki/evaluations/SMOKE.md libs/evals/UNIFIED_EVALS.md
  git commit -m "docs(evals): verify matched Niki benchmark protocol"
  ~~~

---

### Task 4: Capture the first post-Task-11 benchmark baseline

**Files:**
- Modify: docs/niki/evaluations/BASELINE.md
- Create: docs/niki/evaluations/terminal-bench-baseline-comparison.json

**Interfaces:**
- The Harbor comparison JSON is generated by compare_niki_terminal_bench.py from the two complete job directories and records the exact Niki code commit, both job IDs, Harbor/dataset refs, model and effort, sandbox/plugin, agent timeout, attempts, retries, all per-task outcomes, aggregate scores, confidence interval, duration, tokens, and costs where reported.
- The unified lite run uses models=anthropic:claude-opus-5-5, categories=autonomous, agent_impls=dcode, profile=lite, rollouts=3, n_retries=0, agent_timeout_multiplier=1.0, and sandbox_env=langsmith. Dispatch it from the published work branch with --ref <branch> so it evaluates the same branch under review.
- docs/niki/evaluations/BASELINE.md names the capture first post-Task-11 baseline, states that the planned pre-Task-1 baseline was missed, and records the exact Niki commit, workflow run ID, Harbor job IDs, actual cost, all metrics, artifact links, and any missing evidence.

- [ ] **Step 1: Run the unified lite baseline on the evaluated branch**

  Dispatch the existing workflow from the pushed work branch with the fixed settings:
  ~~~bash
  gh workflow run unified_evals.yml --ref <pushed-branch> \
    -f models="anthropic:claude-opus-5-5" \
    -f categories="autonomous" \
    -f agent_impls="dcode" \
    -f profile="lite" \
    -f rollouts="3" \
    -f n_retries="0" \
    -f agent_timeout_multiplier="1.0" \
    -f sandbox_env="langsmith"
  ~~~

  Record the exact workflow commit, run ID, average reward, task coverage, and artifact links. Expected: the run uses Harbor 0.25.0 and completes all three rollouts for every frozen lite-profile task.

- [ ] **Step 2: Run both complete matched Terminal-Bench jobs**

  Run run-niki-terminal-bench and run-claude-code-terminal-bench without MAX_TASKS so Harbor uses the complete pinned task set. Record the Niki product commit before staging dependencies. Confirm each job's config.json matches the fixed protocol and each task has three attempts with matching task names and checksums. If either job is missing expected attempts or task/config sets differ, do not compare it; preserve the evidence and report the baseline as NOT RUN. Recorded failed and timed-out attempts are valid outcomes and stay in the comparison as reward 0.0.

- [ ] **Step 3: Generate the comparison JSON and baseline report**

  Run:
  ~~~bash
  cd libs/evals
  uv run python scripts/compare_niki_terminal_bench.py harbor-jobs/niki-terminal-bench/<job-id> harbor-jobs/claude-code-terminal-bench/<job-id> --out ../../docs/niki/evaluations/terminal-bench-baseline-comparison.json --niki-commit <task-11-product-commit> --harness-commit <evaluation-commit> --baseline-phase first_post_task_11
  ~~~

  Complete BASELINE.md from observed results only. Missing or timed-out attempts remain failures; do not filter tasks or retries after results are observed. If Niki misses either threshold, report the measurement without claiming competitiveness. If credentials or service access prevent a run, mark that evidence NOT RUN, explain exactly what was unavailable, and do not create a JSON comparison artifact from incomplete data.

- [ ] **Step 4: Run final local checks and ownership diff**

  Run:
  ~~~bash
  cd libs/evals
  uv lock --check
  make lint
  uv run pytest tests/unit_tests/test_niki_competitor_eval.py tests/unit_tests/test_harbor_langgraph_agent.py tests/unit_tests/test_harbor_langsmith_integration.py -q
  cd ../..
  git diff --check
  git diff f57c6f383b7018024ca5cde2dc565048ea83202a -- libs/deepagents
  ~~~

  Expected: comparator and model adapter tests pass, the lock matches pyproject.toml, and the libs/deepagents diff is empty.

- [ ] **Step 5: Commit the captured baseline evidence**

  ~~~bash
  git add docs/niki/evaluations/BASELINE.md docs/niki/evaluations/terminal-bench-baseline-comparison.json
  git commit -m "test(code): record matched Niki coding baseline"
  ~~~
