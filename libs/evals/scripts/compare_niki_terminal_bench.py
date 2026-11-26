"""Compare Niki and Claude Code Terminal-Bench results."""

from __future__ import annotations

import argparse
import json
import math
import random
from dataclasses import asdict, dataclass, replace
from datetime import datetime
from pathlib import Path
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from collections.abc import Sequence

TRIALS_PER_TASK = 3
BOOTSTRAP_SAMPLES = 10_000
BOOTSTRAP_SEED = 20261123
NIKI_AGENTS = ("langgraph", "niki-rust")


@dataclass(frozen=True, slots=True)
class TrialResult:
    """One Harbor trial with its validated reward and usage."""

    task_id: str
    task_checksum: str
    trial_id: str
    attempt_index: int
    status: Literal["completed", "failed", "timed_out"]
    reward: float
    duration_seconds: float
    total_tokens: int | None = None
    cost_usd: float | None = None


@dataclass(frozen=True, slots=True)
class TaskOutcome:
    """Paired trial outcomes for one task."""

    task_id: str
    niki_trials: tuple[TrialResult, ...]
    claude_code_trials: tuple[TrialResult, ...]


@dataclass(frozen=True, slots=True)
class ComparisonReport:
    """Aggregate and per-task results of a matched comparison."""

    niki_resolution_rate: float
    claude_code_resolution_rate: float
    difference: float
    difference_ci_95: tuple[float, float]
    task_count: int
    trial_count_per_task: int
    niki_pass_at_1: float
    claude_code_pass_at_1: float
    niki_pass_at_3: float
    claude_code_pass_at_3: float
    niki_median_seconds: float
    niki_p95_seconds: float
    claude_code_median_seconds: float
    claude_code_p95_seconds: float
    niki_cost_per_task_usd: float | None
    claude_code_cost_per_task_usd: float | None
    task_outcomes: tuple[TaskOutcome, ...]


@dataclass(frozen=True, slots=True)
class ComparisonProtocol:
    """The fixed Harbor protocol recorded with a comparison report."""

    benchmark_ref: str
    harbor_version: str
    niki_agent: str
    claude_code_agent: str
    model: str
    reasoning_effort: str
    sandbox: str
    plugin: str
    permission_mode: str
    agent_timeout_multiplier: float
    n_concurrent_trials: int
    retries: int
    trials_per_task: int
    niki_job_id: str
    claude_code_job_id: str
    niki_commit: str
    harness_commit: str
    baseline_phase: Literal["first_post_task_11", "rust_final"]


@dataclass(frozen=True, slots=True)
class _ParsedTrial:
    result: TrialResult
    started_at: datetime
    trial_name: str


def _parse_datetime(value: object) -> datetime:
    if not isinstance(value, str) or not value.strip():
        msg = "missing or invalid timestamp"
        raise ValueError(msg)
    text = value.strip()
    if text.endswith(("Z", "z")):
        text = text[:-1] + "+00:00"
    try:
        return datetime.fromisoformat(text)
    except ValueError as exc:
        msg = f"invalid timestamp {value!r}: {exc}"
        raise ValueError(msg) from exc


def _is_int(value: object) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def _read_trial_json(path: Path) -> dict[str, object]:
    try:
        data = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError, UnicodeDecodeError) as exc:
        msg = f"malformed trial file {path}: {exc}"
        raise ValueError(msg) from exc
    if not isinstance(data, dict):
        msg = f"trial file {path} must contain a JSON object"
        raise ValueError(msg)  # noqa: TRY004 - trial inputs fail as ValueError uniformly
    return data


def _required_text(data: dict[str, object], source: Path, label: str, *keys: str) -> str:
    value = data.get(keys[0])
    if value is None and len(keys) > 1:
        value = data.get(keys[1])
    if not isinstance(value, str) or not value.strip():
        msg = f"missing or invalid {label} in {source}"
        raise ValueError(msg)
    return value.strip()


def _optional_text(data: dict[str, object], source: Path, label: str, key: str) -> str | None:
    value = data.get(key)
    if value is None:
        return None
    if not isinstance(value, str) or not value.strip():
        msg = f"invalid {label} in {source}"
        raise ValueError(msg)
    return value.strip()


def _parse_reward(
    data: dict[str, object], source: Path
) -> tuple[float, Literal["completed", "failed", "timed_out"]]:
    exception_info = data.get("exception_info")
    if exception_info is not None:
        exc_type = ""
        if isinstance(exception_info, dict):
            exc_type = str(exception_info.get("exception_type", ""))
        status: Literal["completed", "failed", "timed_out"] = (
            "timed_out" if "timeout" in exc_type.lower() else "failed"
        )
        return 0.0, status

    verifier = data.get("verifier_result")
    rewards: dict[str, object] = {}
    if isinstance(verifier, dict):
        raw_rewards = verifier.get("rewards")
        if isinstance(raw_rewards, dict):
            rewards = raw_rewards
    reward_raw = rewards.get("reward")
    if reward_raw is None:
        msg = f"missing reward in {source}"
        raise ValueError(msg)
    if isinstance(reward_raw, bool) or not isinstance(reward_raw, (int, float)):
        msg = f"invalid reward in {source}"
        raise ValueError(msg)  # noqa: TRY004 - trial inputs fail as ValueError uniformly
    reward = float(reward_raw)
    if not math.isfinite(reward):
        msg = f"non-finite reward in {source}"
        raise ValueError(msg)
    if not 0.0 <= reward <= 1.0:
        msg = f"reward out of range in {source}"
        raise ValueError(msg)
    return reward, "completed"


def _read_tokens(mapping: dict[str, object], source: Path) -> int | None:
    parts: list[int] = []
    for key in ("n_input_tokens", "n_output_tokens"):
        value = mapping.get(key)
        if value is None:
            continue
        if not _is_int(value) or value < 0:
            msg = f"invalid token count in {source}"
            raise ValueError(msg)
        parts.append(value)
    return sum(parts) if parts else None


def _read_cost(mapping: dict[str, object], source: Path) -> float | None:
    value = mapping.get("cost_usd")
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        msg = f"invalid cost in {source}"
        raise ValueError(msg)  # noqa: TRY004 - trial inputs fail as ValueError uniformly
    cost = float(value)
    if not math.isfinite(cost) or cost < 0:
        msg = f"invalid cost in {source}"
        raise ValueError(msg)
    return cost


def _parse_usage(data: dict[str, object], source: Path) -> tuple[int | None, float | None]:
    agent_result = data.get("agent_result")
    if isinstance(agent_result, dict):
        return _read_tokens(agent_result, source), _read_cost(agent_result, source)
    steps = data.get("step_results")
    if not isinstance(steps, list):
        return None, None
    tokens: list[int] = []
    costs: list[float] = []
    for step in steps:
        if not isinstance(step, dict):
            continue
        step_agent = step.get("agent_result")
        if not isinstance(step_agent, dict):
            continue
        step_tokens = _read_tokens(step_agent, source)
        if step_tokens is not None:
            tokens.append(step_tokens)
        step_cost = _read_cost(step_agent, source)
        if step_cost is not None:
            costs.append(step_cost)
    return (sum(tokens) if tokens else None), (sum(costs) if costs else None)


def _parse_trial_record(data: dict[str, object], source: Path) -> tuple[TrialResult, datetime, str]:
    task_id = _required_text(data, source, "task identity", "task_name", "task_id")
    task_checksum = _required_text(data, source, "task_checksum", "task_checksum")
    trial_id = _required_text(data, source, "trial id", "id", "trial_id")
    trial_name = _optional_text(data, source, "trial_name", "trial_name") or trial_id
    started_at = _parse_datetime(data.get("started_at"))
    finished_at = _parse_datetime(data.get("finished_at"))
    duration = (finished_at - started_at).total_seconds()
    if duration < 0:
        msg = f"negative duration in {source}"
        raise ValueError(msg)
    reward, status = _parse_reward(data, source)
    tokens, cost = _parse_usage(data, source)
    result = TrialResult(
        task_id=task_id,
        task_checksum=task_checksum,
        trial_id=trial_id,
        attempt_index=0,  # assigned after all records for the job are loaded
        status=status,
        reward=reward,
        duration_seconds=duration,
        total_tokens=tokens,
        cost_usd=cost,
    )
    return result, started_at, trial_name


def _trial_files(job_dir: Path) -> list[Path]:
    files = sorted(job_dir.rglob("result.json"))
    job_level = job_dir / "result.json"
    if len(files) > 1 and job_level in files:
        files = [f for f in files if f != job_level]
    if not files:
        files = sorted(job_dir.rglob("trial_result.json"))
    if not files:
        msg = f"no trial result files in {job_dir}"
        raise ValueError(msg)
    return files


def _parse_file(path: Path) -> _ParsedTrial:
    result, started_at, trial_name = _parse_trial_record(_read_trial_json(path), path)
    return _ParsedTrial(result=result, started_at=started_at, trial_name=trial_name)


def _reject_duplicates(parsed: list[_ParsedTrial]) -> None:
    trial_ids: set[str] = set()
    trial_names: set[str] = set()
    for item in parsed:
        if item.result.trial_id in trial_ids:
            msg = f"duplicate trial id {item.result.trial_id}"
            raise ValueError(msg)
        if item.trial_name in trial_names:
            msg = f"duplicate trial name {item.trial_name}"
            raise ValueError(msg)
        trial_ids.add(item.result.trial_id)
        trial_names.add(item.trial_name)


def _assign_attempts(parsed: list[_ParsedTrial]) -> list[TrialResult]:
    by_task: dict[str, list[int]] = {}
    for index, item in enumerate(parsed):
        by_task.setdefault(item.result.task_id, []).append(index)
    for indices in by_task.values():
        indices.sort(key=lambda i: (parsed[i].started_at, parsed[i].trial_name))
        for position, index in enumerate(indices, start=1):
            updated = replace(parsed[index].result, attempt_index=position)
            parsed[index] = replace(parsed[index], result=updated)
    return [item.result for item in parsed]


def load_trial_results(job_dir: Path) -> list[TrialResult]:
    """Load and validate every Harbor trial result in a job directory."""
    parsed = [_parse_file(path) for path in _trial_files(Path(job_dir))]
    _reject_duplicates(parsed)
    return _assign_attempts(parsed)


def _percentile(values: list[float], quantile: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    position = (len(ordered) - 1) * quantile
    lower = math.floor(position)
    upper = math.ceil(position)
    if lower == upper:
        return ordered[lower]
    weight = position - lower
    return ordered[lower] * (1.0 - weight) + ordered[upper] * weight


def _pass_at_k(results: list[TrialResult], k: int) -> float:
    tasks: dict[str, list[TrialResult]] = {}
    for result in results:
        tasks.setdefault(result.task_id, []).append(result)
    if not tasks:
        return 0.0
    successes = 0
    for trials in tasks.values():
        ordered = sorted(trials, key=lambda r: r.attempt_index)
        if any(trial.reward == 1.0 for trial in ordered[:k]):
            successes += 1
    return successes / len(tasks)


def _require_three_attempts(
    niki_trials: list[TrialResult], claude_code_trials: list[TrialResult], task: str
) -> None:
    if len(niki_trials) != TRIALS_PER_TASK or len(claude_code_trials) != TRIALS_PER_TASK:
        msg = f"expected exactly {TRIALS_PER_TASK} attempts for {task}"
        raise ValueError(msg)
    expected = list(range(1, TRIALS_PER_TASK + 1))
    for label, trials in (("Niki", niki_trials), ("Claude Code", claude_code_trials)):
        if sorted(r.attempt_index for r in trials) != expected:
            msg = f"invalid {label} attempt indexes for {task}"
            raise ValueError(msg)


def _require_matching_checksums(
    niki_trials: list[TrialResult], claude_code_trials: list[TrialResult], task: str
) -> None:
    niki_checksums = {r.task_checksum for r in niki_trials}
    claude_checksums = {r.task_checksum for r in claude_code_trials}
    if len(niki_checksums) != 1:
        msg = f"mixed Niki task revisions for {task}"
        raise ValueError(msg)
    if len(claude_checksums) != 1:
        msg = f"mixed Claude Code task revisions for {task}"
        raise ValueError(msg)
    if niki_checksums != claude_checksums:
        msg = f"task checksums differ for {task}"
        raise ValueError(msg)


def _validate_paired_trials(
    niki_by_task: dict[str, list[TrialResult]],
    claude_code_by_task: dict[str, list[TrialResult]],
) -> None:
    niki_tasks = set(niki_by_task)
    claude_tasks = set(claude_code_by_task)
    if not niki_tasks or niki_tasks != claude_tasks:
        msg = "task sets differ between agents"
        raise ValueError(msg)
    for task in niki_tasks:
        niki_trials = niki_by_task[task]
        claude_trials = claude_code_by_task[task]
        _require_three_attempts(niki_trials, claude_trials, task)
        _require_matching_checksums(niki_trials, claude_trials, task)


def _paired_bootstrap_ci(
    niki_by_task: dict[str, list[TrialResult]],
    claude_code_by_task: dict[str, list[TrialResult]],
    *,
    bootstrap_samples: int,
    seed: int,
) -> tuple[float, float]:
    task_ids = sorted(niki_by_task)
    rng = random.Random(seed)
    diffs: list[float] = []
    for _ in range(bootstrap_samples):
        sampled_tasks = [rng.choice(task_ids) for _ in task_ids]
        n_success = 0
        c_success = 0
        for task in sampled_tasks:
            for result in niki_by_task[task]:
                n_success += int(result.reward == 1.0)
            for result in claude_code_by_task[task]:
                c_success += int(result.reward == 1.0)
        total = TRIALS_PER_TASK * len(task_ids)
        diffs.append(n_success / total - c_success / total)
    diffs.sort()
    return _percentile(diffs, 0.025), _percentile(diffs, 0.975)


def _group_by_task(results: Sequence[TrialResult]) -> dict[str, list[TrialResult]]:
    grouped: dict[str, list[TrialResult]] = {}
    for result in results:
        grouped.setdefault(result.task_id, []).append(result)
    return grouped


def _resolution_rate(results: list[TrialResult]) -> float:
    return sum(1 for result in results if result.reward == 1.0) / len(results)


def _build_report(
    niki: list[TrialResult],
    claude_code: list[TrialResult],
    niki_by_task: dict[str, list[TrialResult]],
    claude_code_by_task: dict[str, list[TrialResult]],
    ci_95: tuple[float, float],
) -> ComparisonReport:
    niki_durations = [result.duration_seconds for result in niki]
    cc_durations = [result.duration_seconds for result in claude_code]
    niki_costs = [result.cost_usd for result in niki if result.cost_usd is not None]
    cc_costs = [result.cost_usd for result in claude_code if result.cost_usd is not None]
    outcomes = tuple(
        TaskOutcome(
            task_id=task,
            niki_trials=tuple(sorted(niki_by_task[task], key=lambda r: r.attempt_index)),
            claude_code_trials=tuple(
                sorted(claude_code_by_task[task], key=lambda r: r.attempt_index)
            ),
        )
        for task in sorted(niki_by_task)
    )
    niki_rate = _resolution_rate(niki)
    cc_rate = _resolution_rate(claude_code)
    return ComparisonReport(
        niki_resolution_rate=niki_rate,
        claude_code_resolution_rate=cc_rate,
        difference=niki_rate - cc_rate,
        difference_ci_95=ci_95,
        task_count=len(niki_by_task),
        trial_count_per_task=TRIALS_PER_TASK,
        niki_pass_at_1=_pass_at_k(niki, 1),
        claude_code_pass_at_1=_pass_at_k(claude_code, 1),
        niki_pass_at_3=_pass_at_k(niki, TRIALS_PER_TASK),
        claude_code_pass_at_3=_pass_at_k(claude_code, TRIALS_PER_TASK),
        niki_median_seconds=_percentile(niki_durations, 0.5),
        niki_p95_seconds=_percentile(niki_durations, 0.95),
        claude_code_median_seconds=_percentile(cc_durations, 0.5),
        claude_code_p95_seconds=_percentile(cc_durations, 0.95),
        niki_cost_per_task_usd=(sum(niki_costs) / len(niki_costs)) if niki_costs else None,
        claude_code_cost_per_task_usd=(sum(cc_costs) / len(cc_costs)) if cc_costs else None,
        task_outcomes=outcomes,
    )


def compare_results(
    niki: Sequence[TrialResult],
    claude_code: Sequence[TrialResult],
    *,
    bootstrap_samples: int = BOOTSTRAP_SAMPLES,
    seed: int = BOOTSTRAP_SEED,
) -> ComparisonReport:
    """Compare two complete, paired trial sets with a paired bootstrap interval."""
    if bootstrap_samples <= 0:
        msg = "bootstrap_samples must be positive"
        raise ValueError(msg)
    niki_trials = list(niki)
    claude_trials = list(claude_code)
    niki_by_task = _group_by_task(niki_trials)
    claude_by_task = _group_by_task(claude_trials)
    _validate_paired_trials(niki_by_task, claude_by_task)
    ci_95 = _paired_bootstrap_ci(
        niki_by_task, claude_by_task, bootstrap_samples=bootstrap_samples, seed=seed
    )
    return _build_report(niki_trials, claude_trials, niki_by_task, claude_by_task, ci_95)


def write_comparison_json(
    report: ComparisonReport,
    protocol: ComparisonProtocol,
    *,
    output_path: Path,
) -> None:
    """Write the protocol and complete report to a JSON file."""
    output_path = Path(output_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    payload = {"protocol": asdict(protocol), "report": asdict(report)}
    output_path.write_text(json.dumps(payload, indent=2, default=str))


def _job_id(job_dir: Path) -> str:
    result_path = Path(job_dir) / "result.json"
    if not result_path.exists():
        msg = f"missing job-level result.json in {job_dir}"
        raise ValueError(msg)
    try:
        data = json.loads(result_path.read_text())
    except (OSError, json.JSONDecodeError, UnicodeDecodeError) as exc:
        msg = f"malformed job-level result.json in {job_dir}: {exc}"
        raise ValueError(msg) from exc
    if not isinstance(data, dict):
        msg = f"job-level result.json in {job_dir} must be an object"
        raise ValueError(msg)  # noqa: TRY004 - job inputs fail as ValueError uniformly
    value = data.get("id", data.get("job_id"))
    if value is None or (isinstance(value, str) and not value.strip()):
        msg = f"missing job id in {job_dir}"
        raise ValueError(msg)
    return str(value).strip()


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("niki_job_dir")
    parser.add_argument("claude_code_job_dir")
    parser.add_argument("--out", default=None)
    parser.add_argument("--niki-commit", default="")
    parser.add_argument("--harness-commit", default="")
    parser.add_argument("--baseline-phase", default="first_post_task_11")
    parser.add_argument("--niki-agent", required=True, choices=NIKI_AGENTS)
    return parser


def _protocol(
    args: argparse.Namespace, report: ComparisonReport, niki_dir: Path, cc_dir: Path
) -> ComparisonProtocol:
    return ComparisonProtocol(
        benchmark_ref="terminal-bench/terminal-bench@4.1.0",
        harbor_version="0.25.0",
        niki_agent=args.niki_agent,
        claude_code_agent="claude-code",
        model="anthropic:claude-opus-5-5",
        reasoning_effort="max",
        sandbox="langsmith",
        plugin="langsmith",
        permission_mode="bypassPermissions",
        agent_timeout_multiplier=1.0,
        n_concurrent_trials=1,
        retries=0,
        trials_per_task=report.trial_count_per_task,
        niki_job_id=_job_id(niki_dir),
        claude_code_job_id=_job_id(cc_dir),
        niki_commit=args.niki_commit,
        harness_commit=args.harness_commit,
        baseline_phase=args.baseline_phase,
    )


def main() -> None:
    """Run the comparator CLI and emit the complete JSON report."""
    args = _build_parser().parse_args()
    niki_dir = Path(args.niki_job_dir)
    cc_dir = Path(args.claude_code_job_dir)
    report = compare_results(load_trial_results(niki_dir), load_trial_results(cc_dir))
    protocol = _protocol(args, report, niki_dir, cc_dir)
    payload = {"protocol": asdict(protocol), "report": asdict(report)}
    text = json.dumps(payload, indent=2, default=str)
    if args.out:
        write_comparison_json(report, protocol, output_path=Path(args.out))
    print(text)


if __name__ == "__main__":
    main()
