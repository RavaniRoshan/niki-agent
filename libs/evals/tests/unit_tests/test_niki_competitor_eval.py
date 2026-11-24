"""Tests for the Niki competitor eval comparison."""

from __future__ import annotations

import json
import shlex
import subprocess
import sys
from pathlib import Path

import pytest
import yaml

from scripts.compare_niki_terminal_bench import (
    TRIALS_PER_TASK,
    TrialResult,
    _percentile,
    compare_results,
    load_trial_results,
    main,
)

ROOT = Path(__file__).resolve().parents[4]
EVALS_DIR = Path(__file__).resolve().parents[2]
FIXTURE_DIR = EVALS_DIR / "tests" / "fixtures" / "harbor-0.25" / "terminal-bench-trial"


def _record(
    *,
    task: str = "t1",
    trial: str = "r1",
    checksum: str = "checksum",
    reward: float = 1.0,
    started_at: str = "2026-01-01T00:00:00Z",
    finished_at: str = "2026-01-01T00:00:01Z",
    tokens: int | None = 1,
    cost: float | None = 0.1,
    trial_name: str | None = None,
    include_reward: bool = True,
    exception_info: object = None,
) -> dict[str, object]:
    verifier: dict[str, object] = {"rewards": {"reward": reward}} if include_reward else {}
    data: dict[str, object] = {
        "task_name": task,
        "id": trial,
        "task_checksum": checksum,
        "verifier_result": verifier,
        "exception_info": exception_info,
        "started_at": started_at,
        "finished_at": finished_at,
    }
    if trial_name is not None:
        data["trial_name"] = trial_name
    agent_result: dict[str, object] = {}
    if tokens is not None:
        agent_result["n_input_tokens"] = tokens
        agent_result["n_output_tokens"] = tokens
    if cost is not None:
        agent_result["cost_usd"] = cost
    data["agent_result"] = agent_result
    return data


def _write_data(job_dir: Path, data: object, *, name: str = "trial") -> Path:
    directory = job_dir / name
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / "result.json"
    path.write_text(data if isinstance(data, str) else json.dumps(data))
    return path


def _write_trial(
    job_dir: Path, task: str, trial: str, reward: float, checksum: str = "checksum"
) -> Path:
    return _write_data(
        job_dir,
        _record(task=task, trial=trial, reward=reward, checksum=checksum),
        name=f"{task}__{trial}",
    )


def _job_dir(tmp_path: Path, name: str) -> Path:
    job_dir = tmp_path / name
    job_dir.mkdir()
    return job_dir


def test_paired_comparison_rejects_incomplete_or_mismatched_task_trials(tmp_path):
    niki_dir = _job_dir(tmp_path, "niki")
    cc_dir = _job_dir(tmp_path, "cc")
    _write_trial(niki_dir, "t1", "r1", 1.0)
    _write_trial(cc_dir, "t1", "r1", 1.0)
    _write_trial(niki_dir, "t2", "r2", 1.0)
    with pytest.raises(ValueError, match="task sets differ between agents"):
        compare_results(load_trial_results(niki_dir), load_trial_results(cc_dir))


def test_paired_bootstrap_reports_zero_for_identical_agents(tmp_path):
    niki_dir = _job_dir(tmp_path, "niki")
    cc_dir = _job_dir(tmp_path, "cc")
    for i in range(TRIALS_PER_TASK):
        _write_trial(niki_dir, "t1", f"r{i}", 1.0)
        _write_trial(cc_dir, "t1", f"r{i}", 1.0)
    report = compare_results(load_trial_results(niki_dir), load_trial_results(cc_dir))
    assert report.difference == 0.0
    assert report.difference_ci_95 == (0.0, 0.0)


@pytest.mark.parametrize(
    ("mutate", "message"),
    [
        (lambda d: d.pop("task_name"), "missing or invalid task identity"),
        (lambda d: d.pop("task_checksum"), "missing or invalid task_checksum"),
        (lambda d: d.pop("id"), "missing or invalid trial id"),
        (lambda d: d.pop("started_at"), "missing or invalid timestamp"),
        (lambda d: d.pop("finished_at"), "missing or invalid timestamp"),
        (
            lambda d: d.__setitem__("verifier_result", {"rewards": {"reward": float("nan")}}),
            "non-finite reward",
        ),
        (
            lambda d: d.__setitem__("verifier_result", {"rewards": {"reward": float("inf")}}),
            "non-finite reward",
        ),
        (
            lambda d: d.__setitem__("verifier_result", {"rewards": {"reward": -0.1}}),
            "reward out of range",
        ),
        (
            lambda d: d.__setitem__("verifier_result", {"rewards": {"reward": 1.1}}),
            "reward out of range",
        ),
        (
            lambda d: d.__setitem__("verifier_result", {"rewards": {"reward": True}}),
            "invalid reward",
        ),
        (lambda d: d.__setitem__("verifier_result", {}), "missing reward"),
        (lambda d: d.__setitem__("finished_at", "2025-12-31T23:59:59Z"), "negative duration"),
        (lambda d: d.__setitem__("agent_result", {"n_input_tokens": -1}), "invalid token count"),
        (lambda d: d.__setitem__("agent_result", {"cost_usd": -0.1}), "invalid cost"),
    ],
)
def test_load_trial_results_rejects_invalid_records(tmp_path, mutate, message):
    data = json.loads((FIXTURE_DIR / "result.json").read_text())
    mutate(data)
    job_dir = _job_dir(tmp_path, "trial")
    _write_data(job_dir, data)
    with pytest.raises(ValueError, match=message):
        load_trial_results(job_dir)


def test_load_trial_results_rejects_malformed_json(tmp_path):
    job_dir = _job_dir(tmp_path, "trial")
    _write_data(job_dir, "{not json")
    with pytest.raises(ValueError, match="malformed trial file"):
        load_trial_results(job_dir)


def test_load_trial_results_rejects_duplicate_trial_ids(tmp_path):
    job_dir = _job_dir(tmp_path, "trial")
    _write_data(job_dir, _record(trial="same", trial_name="first"), name="first")
    _write_data(job_dir, _record(trial="same", trial_name="second"), name="second")
    with pytest.raises(ValueError, match="duplicate trial id same"):
        load_trial_results(job_dir)


def test_load_trial_results_rejects_duplicate_trial_names(tmp_path):
    job_dir = _job_dir(tmp_path, "trial")
    _write_data(job_dir, _record(trial="first", trial_name="same"), name="first")
    _write_data(job_dir, _record(trial="second", trial_name="same"), name="second")
    with pytest.raises(ValueError, match="duplicate trial name same"):
        load_trial_results(job_dir)


@pytest.mark.parametrize(
    ("exception_type", "status"),
    [
        ("TimeoutError", "timed_out"),
        ("ValueError", "failed"),
        ({}, "failed"),
    ],
)
def test_load_trial_results_accepts_failure_without_reward(tmp_path, exception_type, status):
    job_dir = _job_dir(tmp_path, "trial")
    _write_data(
        job_dir,
        _record(include_reward=False, exception_info={"exception_type": exception_type}),
    )
    results = load_trial_results(job_dir)
    assert results[0].reward == 0.0
    assert results[0].status == status


def test_load_trial_results_reads_harbor_025_fixture():
    results = load_trial_results(FIXTURE_DIR)
    assert len(results) == 1
    assert results[0].task_id == "terminal-bench-trial"
    assert results[0].task_checksum == "abc123checksum"
    assert results[0].reward == 1.0
    assert results[0].status == "completed"


@pytest.mark.parametrize("attempts", [2, 4])
def test_attempt_indexes_are_assigned_in_start_order(tmp_path, attempts):
    job_dir = _job_dir(tmp_path, "trial")
    for index in range(attempts):
        minute = f"{index:02d}"
        _write_data(
            job_dir,
            _record(
                trial=f"r{index}",
                started_at=f"2026-01-01T00:{minute}:00Z",
                finished_at=f"2026-01-01T00:{minute}:01Z",
            ),
            name=f"trial-{index}",
        )
    results = load_trial_results(job_dir)
    assert [result.attempt_index for result in results] == list(range(1, attempts + 1))


def test_comparison_rejects_two_or_four_attempts(tmp_path):
    for attempts in (2, 4):
        niki_dir = _job_dir(tmp_path, f"niki-{attempts}")
        cc_dir = _job_dir(tmp_path, f"cc-{attempts}")
        for index in range(attempts):
            _write_trial(niki_dir, "t1", f"n{index}", 1.0)
            _write_trial(cc_dir, "t1", f"c{index}", 1.0)
        with pytest.raises(ValueError, match="expected exactly 3 attempts"):
            compare_results(load_trial_results(niki_dir), load_trial_results(cc_dir))


def test_comparison_rejects_duplicate_attempt_indexes():
    niki = [
        TrialResult("t1", "checksum", "n1", 1, "completed", 1.0, 1.0),
        TrialResult("t1", "checksum", "n2", 1, "completed", 1.0, 1.0),
        TrialResult("t1", "checksum", "n3", 3, "completed", 1.0, 1.0),
    ]
    cc = [
        TrialResult("t1", "checksum", "c1", 1, "completed", 1.0, 1.0),
        TrialResult("t1", "checksum", "c2", 2, "completed", 1.0, 1.0),
        TrialResult("t1", "checksum", "c3", 3, "completed", 1.0, 1.0),
    ]
    with pytest.raises(ValueError, match="invalid Niki attempt indexes"):
        compare_results(niki, cc)


def test_paired_comparison_rejects_revision_drift_within_agent(tmp_path):
    niki_dir = _job_dir(tmp_path, "niki")
    cc_dir = _job_dir(tmp_path, "cc")
    for index, checksum in enumerate(["rev-A", "rev-B", "rev-A"]):
        _write_trial(niki_dir, "t1", f"n{index}", 1.0, checksum)
    for index in range(TRIALS_PER_TASK):
        _write_trial(cc_dir, "t1", f"c{index}", 1.0, "rev-A")
    with pytest.raises(ValueError, match="mixed Niki task revisions"):
        compare_results(load_trial_results(niki_dir), load_trial_results(cc_dir))


def test_paired_comparison_rejects_cross_agent_checksum_mismatch(tmp_path):
    niki_dir = _job_dir(tmp_path, "niki")
    cc_dir = _job_dir(tmp_path, "cc")
    for index in range(TRIALS_PER_TASK):
        _write_trial(niki_dir, "t1", f"n{index}", 1.0, "rev-A")
        _write_trial(cc_dir, "t1", f"c{index}", 1.0, "rev-B")
    with pytest.raises(ValueError, match="task checksums differ"):
        compare_results(load_trial_results(niki_dir), load_trial_results(cc_dir))


def test_report_uses_linear_interpolated_duration_percentiles():
    assert _percentile([0, 10, 20, 30], 0.5) == 15
    assert _percentile([0, 10, 20, 30], 0.95) == pytest.approx(28.5)


def _build_job(tmp_path: Path, name: str, rewards: list[float]) -> Path:
    job_dir = _job_dir(tmp_path, name)
    (job_dir / "result.json").write_text(json.dumps({"id": f"job-{name}"}))
    for index, reward in enumerate(rewards):
        _write_trial(job_dir, "t1", f"{name}-{index}", reward)
    return job_dir


def test_comparison_cli_requires_job_ids(tmp_path):
    niki_dir = _job_dir(tmp_path, "niki")
    cc_dir = _job_dir(tmp_path, "cc")
    for index in range(TRIALS_PER_TASK):
        _write_trial(niki_dir, "t1", f"n{index}", 1.0)
        _write_trial(cc_dir, "t1", f"c{index}", 1.0)
    old = sys.argv
    sys.argv = [
        "x",
        str(niki_dir),
        str(cc_dir),
        "--niki-agent",
        "langgraph",
        "--out",
        str(tmp_path / "out.json"),
    ]
    try:
        with pytest.raises(ValueError, match=r"missing job-level result\.json"):
            main()
    finally:
        sys.argv = old


def test_comparison_json_contains_protocol_and_per_task_trials(tmp_path):
    niki_dir = _build_job(tmp_path, "niki", [1.0, 0.0, 1.0])
    cc_dir = _build_job(tmp_path, "cc", [0.0, 0.0, 1.0])
    output = tmp_path / "report.json"
    old = sys.argv
    sys.argv = [
        "x",
        str(niki_dir),
        str(cc_dir),
        "--niki-agent",
        "langgraph",
        "--niki-commit",
        "a" * 40,
        "--harness-commit",
        "b" * 40,
        "--out",
        str(output),
    ]
    try:
        main()
    finally:
        sys.argv = old
    payload = json.loads(output.read_text())
    protocol = payload["protocol"]
    report = payload["report"]
    assert protocol["niki_agent"] == "langgraph"
    assert protocol["claude_code_agent"] == "claude-code"
    assert protocol["benchmark_ref"] == "terminal-bench/terminal-bench@4.1.0"
    assert protocol["harbor_version"] == "0.25.0"
    assert protocol["niki_job_id"] == "job-niki"
    assert protocol["claude_code_job_id"] == "job-cc"
    assert protocol["niki_commit"] == "a" * 40
    assert protocol["harness_commit"] == "b" * 40
    assert protocol["baseline_phase"] == "first_post_task_11"
    assert protocol["model"] == "anthropic:claude-opus-5-5"
    assert protocol["reasoning_effort"] == "max"
    assert protocol["sandbox"] == "langsmith"
    assert protocol["plugin"] == "langsmith"
    assert protocol["permission_mode"] == "bypassPermissions"
    assert protocol["agent_timeout_multiplier"] == 1.0
    assert protocol["n_concurrent_trials"] == 1
    assert protocol["retries"] == 0
    assert report["task_count"] == 1
    assert report["trial_count_per_task"] == TRIALS_PER_TASK
    outcome = report["task_outcomes"][0]
    assert outcome["task_id"] == "t1"
    assert len(outcome["niki_trials"]) == TRIALS_PER_TASK
    assert len(outcome["claude_code_trials"]) == TRIALS_PER_TASK
    assert all(trial["task_checksum"] == "checksum" for trial in outcome["niki_trials"])
    assert all(trial["task_checksum"] == "checksum" for trial in outcome["claude_code_trials"])
    assert report["niki_resolution_rate"] == 2 / 3
    assert report["claude_code_resolution_rate"] == 1 / 3
    assert report["difference"] == 1 / 3
    assert report["niki_pass_at_1"] == 1.0
    assert report["claude_code_pass_at_1"] == 0.0
    assert report["niki_pass_at_3"] == 1.0
    assert report["claude_code_pass_at_3"] == 1.0
    assert report["niki_median_seconds"] == 1.0
    assert report["niki_p95_seconds"] == 1.0
    assert report["niki_cost_per_task_usd"] == pytest.approx(0.1)


def _rendered_make_command(target: str, *make_vars: str) -> list[str]:
    proc = subprocess.run(
        ["make", "-n", target, *make_vars],
        check=True,
        capture_output=True,
        text=True,
        cwd=EVALS_DIR,
    )
    lines = proc.stdout.splitlines()
    start = next(i for i, line in enumerate(lines) if "harbor run" in line)
    collected = []
    for line in lines[start:]:
        collected.append(line.rstrip("\\").strip())
        if not line.rstrip().endswith("\\"):
            break
    return shlex.split(" ".join(collected))


def _option(command: list[str], name: str) -> list[str]:
    return [
        command[i + 1] for i, value in enumerate(command) if value == name and i + 1 < len(command)
    ]


@pytest.mark.parametrize("make_vars", [(), ("MAX_TASKS=2",)])
def test_harbor_runner_targets_use_same_terminal_bench_inputs(make_vars):
    niki_cmd = _rendered_make_command("run-niki-terminal-bench", *make_vars)
    cc_cmd = _rendered_make_command("run-claude-code-terminal-bench", *make_vars)
    assert "--agent" in niki_cmd
    assert niki_cmd[niki_cmd.index("--agent") + 1] == "langgraph"
    assert "--agent" in cc_cmd
    assert cc_cmd[cc_cmd.index("--agent") + 1] == "claude-code"
    assert "graph=dcode" in niki_cmd
    assert "permission_mode=bypassPermissions" in cc_cmd
    assert (
        _option(niki_cmd, "--config") == _option(cc_cmd, "--config") == ["terminal-bench-4.1.yaml"]
    )
    assert (
        _option(niki_cmd, "--dataset")
        == _option(cc_cmd, "--dataset")
        == ["terminal-bench/terminal-bench@4.1.0"]
    )
    assert (
        _option(niki_cmd, "--model") == _option(cc_cmd, "--model") == ["anthropic/claude-opus-5-5"]
    )
    assert _option(niki_cmd, "--env") == _option(cc_cmd, "--env") == ["langsmith"]
    assert _option(niki_cmd, "--plugin") == _option(cc_cmd, "--plugin") == ["langsmith"]
    assert (
        _option(niki_cmd, "--agent-timeout-multiplier")
        == _option(cc_cmd, "--agent-timeout-multiplier")
        == ["1.0"]
    )
    assert _option(niki_cmd, "--max-retries") == _option(cc_cmd, "--max-retries") == ["0"]
    assert "dataset_name=terminal-bench/terminal-bench@4.1.0" in niki_cmd
    assert "dataset_name=terminal-bench/terminal-bench@4.1.0" in cc_cmd
    assert "harbor-jobs/niki-terminal-bench/" in " ".join(niki_cmd)
    assert "harbor-jobs/claude-code-terminal-bench/" in " ".join(cc_cmd)
    if make_vars:
        assert _option(niki_cmd, "--n-tasks") == _option(cc_cmd, "--n-tasks") == ["2"]
    else:
        assert _option(niki_cmd, "--n-tasks") == _option(cc_cmd, "--n-tasks") == []

    config = yaml.safe_load((EVALS_DIR / "terminal-bench-4.1.yaml").read_text())
    assert config["n_attempts"] == TRIALS_PER_TASK
    assert config["n_concurrent_trials"] == 1
    assert config["retry"]["max_retries"] == 0
    assert config["agent_timeout_multiplier"] == 1.0
    assert config["environment"]["type"] == "langsmith"


def test_niki_smoke_workflow_is_valid_and_secret_safe():
    workflow_path = ROOT / ".github" / "workflows" / "niki-smoke.yml"
    text = workflow_path.read_text()
    data = yaml.load(text, Loader=yaml.BaseLoader)
    assert "workflow_dispatch" in data.get("on", {})
    assert "smoke" in data["jobs"]
    for line in text.splitlines():
        if "uses:" in line:
            ref = line.split("uses:")[-1].strip()
            if "@" in ref:
                sha = ref.split("@")[-1]
                assert len(sha) == 40
    assert "gsk_" not in text
