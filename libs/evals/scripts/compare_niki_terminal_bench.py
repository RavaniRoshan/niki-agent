"""Compare Niki and Claude Code Terminal-Bench results."""

from __future__ import annotations

import argparse
import json
import random
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Sequence


@dataclass(frozen=True, slots=True)
class TrialResult:
    task_id: str
    trial_id: str
    reward: float
    duration_seconds: float
    total_tokens: int | None = None
    cost_usd: float | None = None


@dataclass(frozen=True, slots=True)
class ComparisonReport:
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


def load_trial_results(job_dir: Path) -> list[TrialResult]:
    job_dir = Path(job_dir)
    results: list[TrialResult] = []
    seen: set[tuple[str, str]] = set()
    files = sorted(job_dir.rglob("trial_result.json"))
    if not files:
        # fallback: result.json
        files = sorted(job_dir.rglob("result.json"))
    if not files:
        raise ValueError(f"no trial_result.json files in {job_dir}")
    for f in files:
        try:
            data = json.loads(f.read_text())
        except Exception as exc:
            raise ValueError(f"malformed trial file {f}: {exc}") from exc
        task_id = data.get("task_id") or data.get("task_name")
        trial_id = data.get("trial_id") or f.parent.name
        if task_id is None or trial_id is None:
            raise ValueError(f"missing task/trial id in {f}")
        key = (str(task_id), str(trial_id))
        if key in seen:
            raise ValueError(f"duplicate trial id {key}")
        seen.add(key)
        reward = data.get("reward")
        if reward is None:
            vr = data.get("verifier_result") or {}
            rewards = vr.get("rewards", {}) if isinstance(vr, dict) else {}
            reward = rewards.get("reward", 0.0)
        duration = data.get("duration_seconds") or data.get("duration") or 0.0
        tokens = data.get("total_tokens")
        cost = data.get("cost_usd") or data.get("cost")
        results.append(
            TrialResult(
                task_id=str(task_id),
                trial_id=str(trial_id),
                reward=float(reward),
                duration_seconds=float(duration),
                total_tokens=int(tokens) if tokens is not None else None,
                cost_usd=float(cost) if cost is not None else None,
            )
        )
    return results


def _median(xs: list[float]) -> float:
    if not xs:
        return 0.0
    s = sorted(xs)
    n = len(s)
    if n % 2:
        return s[n // 2]
    return (s[n // 2 - 1] + s[n // 2]) / 2.0


def _p95(xs: list[float]) -> float:
    if not xs:
        return 0.0
    s = sorted(xs)
    idx = max(0, int(0.95 * (len(s) - 1)))
    return s[idx]


def _pass_at_k(results: list[TrialResult], k: int) -> float:
    tasks: dict[str, list[TrialResult]] = {}
    for r in results:
        tasks.setdefault(r.task_id, []).append(r)
    if not tasks:
        return 0.0
    successes = 0
    for tid, trials in tasks.items():
        if any(t.reward == 1.0 for t in trials[:k]):
            successes += 1
    return successes / len(tasks)


def compare_results(
    niki: Sequence[TrialResult],
    claude_code: Sequence[TrialResult],
    *,
    bootstrap_samples: int = 10_000,
    seed: int = 20261123,
) -> ComparisonReport:
    niki = list(niki)
    claude_code = list(claude_code)

    niki_tasks = {r.task_id for r in niki}
    cc_tasks = {r.task_id for r in claude_code}
    if niki_tasks != cc_tasks:
        raise ValueError("task sets differ between agents")

    # trial counts per task
    niki_counts: dict[str, int] = {}
    cc_counts: dict[str, int] = {}
    for r in niki:
        niki_counts[r.task_id] = niki_counts.get(r.task_id, 0) + 1
    for r in claude_code:
        cc_counts[r.task_id] = cc_counts.get(r.task_id, 0) + 1
    if niki_counts != cc_counts:
        raise ValueError("trial counts differ between agents")

    trial_count_per_task = next(iter(niki_counts.values())) if niki_counts else 0

    niki_rate = (sum(1 for r in niki if r.reward == 1.0) / len(niki)) if niki else 0.0
    cc_rate = (sum(1 for r in claude_code if r.reward == 1.0) / len(claude_code)) if claude_code else 0.0

    # bootstrap over shared task ids
    task_ids = sorted(niki_tasks)
    niki_by_task: dict[str, list[TrialResult]] = {t: [] for t in task_ids}
    cc_by_task: dict[str, list[TrialResult]] = {t: [] for t in task_ids}
    for r in niki:
        niki_by_task[r.task_id].append(r)
    for r in claude_code:
        cc_by_task[r.task_id].append(r)

    rng = random.Random(seed)
    diffs = []
    for _ in range(bootstrap_samples):
        sampled_tasks = [rng.choice(task_ids) for _ in task_ids]
        n_succ = 0
        c_succ = 0
        n_tot = 0
        c_tot = 0
        for t in sampled_tasks:
            for r in niki_by_task[t]:
                n_tot += 1
                if r.reward == 1.0:
                    n_succ += 1
            for r in cc_by_task[t]:
                c_tot += 1
                if r.reward == 1.0:
                    c_succ += 1
        nr = n_succ / n_tot if n_tot else 0.0
        cr = c_succ / c_tot if c_tot else 0.0
        diffs.append(nr - cr)
    diffs.sort()
    lo = diffs[int(0.025 * len(diffs))]
    hi = diffs[int(0.975 * len(diffs))]

    niki_durs = [r.duration_seconds for r in niki]
    cc_durs = [r.duration_seconds for r in claude_code]

    niki_costs = [r.cost_usd for r in niki if r.cost_usd is not None]
    cc_costs = [r.cost_usd for r in claude_code if r.cost_usd is not None]

    return ComparisonReport(
        niki_resolution_rate=niki_rate,
        claude_code_resolution_rate=cc_rate,
        difference=niki_rate - cc_rate,
        difference_ci_95=(lo, hi),
        task_count=len(task_ids),
        trial_count_per_task=trial_count_per_task,
        niki_pass_at_1=_pass_at_k(niki, 1),
        claude_code_pass_at_1=_pass_at_k(claude_code, 1),
        niki_pass_at_3=_pass_at_k(niki, 3),
        claude_code_pass_at_3=_pass_at_k(claude_code, 3),
        niki_median_seconds=_median(niki_durs),
        niki_p95_seconds=_p95(niki_durs),
        claude_code_median_seconds=_median(cc_durs),
        claude_code_p95_seconds=_p95(cc_durs),
        niki_cost_per_task_usd=(sum(niki_costs) / len(niki_costs)) if niki_costs else None,
        claude_code_cost_per_task_usd=(sum(cc_costs) / len(cc_costs)) if cc_costs else None,
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("niki_job_dir")
    parser.add_argument("claude_code_job_dir")
    parser.add_argument("--out", default=None)
    args = parser.parse_args()
    niki = load_trial_results(Path(args.niki_job_dir))
    cc = load_trial_results(Path(args.claude_code_job_dir))
    report = compare_results(niki, cc)
    payload = asdict(report)
    text = json.dumps(payload, indent=2, default=str)
    if args.out:
        Path(args.out).write_text(text)
    print(text)


if __name__ == "__main__":
    main()
