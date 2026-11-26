"""Dynamic workflow controller for Niki product completion."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, Literal, cast

if TYPE_CHECKING:
    from collections.abc import Sequence

Priority = Literal["P0", "P1"]
WorkflowStatus = Literal[
    "WORKS", "PARTIAL", "MISSING", "BROKEN", "NOT STARTED", "DEFERRED"
]
Phase = Literal["python", "python_evaluation", "rust", "rust_evaluation", "complete"]
PhaseGroup = Literal["python", "rust"]

PHASES: tuple[Phase, ...] = (
    "python",
    "python_evaluation",
    "rust",
    "rust_evaluation",
    "complete",
)
IMPLEMENTATION_PHASES: tuple[Phase, ...] = ("python", "rust")
EVALUATION_PHASES: tuple[Phase, ...] = ("python_evaluation", "rust_evaluation")
STATUSES: tuple[WorkflowStatus, ...] = (
    "WORKS",
    "PARTIAL",
    "MISSING",
    "BROKEN",
    "NOT STARTED",
    "DEFERRED",
)
TRIALS_PER_TASK = 3
CI_BOUNDS = 2
MIN_RESOLUTION_RATE = 0.55
MIN_CI_LOWER = -0.10
MAX_AVG_AT_3_DROP = 0.05
COMMIT_SHA_LENGTH = 40
BENCHMARK_REF = "terminal-bench/terminal-bench@4.1.0"
HARBOR_VERSION = "0.25.0"

_CHECKLIST_ENV = "NIKI_WORKFLOW_CHECKLIST"
_STATE_ENV = "NIKI_WORKFLOW_STATE"
_REPO_ROOT = Path(__file__).resolve().parents[3]


@dataclass(frozen=True, slots=True)
class WorkItem:
    """One product requirement parsed from a checklist row."""

    item_id: str
    title: str
    priority: Priority
    python_status: WorkflowStatus
    python_evidence: str
    rust_status: WorkflowStatus
    rust_evidence: str
    source_row: str


@dataclass(slots=True)
class WorkflowState:
    """Orchestration state stored in docs/niki/workflow-state.json."""

    phase: Phase = "python"
    active_item: str | None = None
    active_step: str | None = None
    item_steps: dict[str, str] = field(default_factory=dict)
    owner: str | None = None
    branch: str | None = None
    blocked: dict[str, str] = field(default_factory=dict)
    last_commit: str | None = None
    last_eval: dict[str, object] | None = None
    completed: dict[str, dict[str, str]] = field(default_factory=dict)

    def to_dict(self) -> dict[str, object]:
        """Return a JSON-serialisable representation."""
        return asdict(self)

    @classmethod
    def from_dict(cls, data: dict[str, object]) -> WorkflowState:
        """Build state from a JSON object, ignoring unknown keys.

        Returns:
            The reconstructed workflow state.
        """
        phase = data.get("phase")
        return cls(
            phase=cast("Phase", phase) if phase in PHASES else "python",
            active_item=_optional_str(data.get("active_item")),
            active_step=_optional_str(data.get("active_step")),
            item_steps=_str_dict(data.get("item_steps")),
            owner=_optional_str(data.get("owner")),
            branch=_optional_str(data.get("branch")),
            blocked=_str_dict(data.get("blocked")),
            last_commit=_optional_str(data.get("last_commit")),
            last_eval=_object_dict(data.get("last_eval")),
            completed={
                key: _str_dict(value)
                for key, value in _object_dict(data.get("completed")).items()
            },
        )


def _optional_str(value: object) -> str | None:
    return value if isinstance(value, str) and value else None


def _str_dict(value: object) -> dict[str, str]:
    if not isinstance(value, dict):
        return {}
    return {str(key): str(item) for key, item in value.items()}


def _object_dict(value: object) -> dict[str, object]:
    if not isinstance(value, dict):
        return {}
    return {str(key): item for key, item in value.items()}


def _status(value: str) -> WorkflowStatus:
    normalised = value.replace("*", "").strip()
    for candidate in STATUSES:
        if normalised.startswith(candidate):
            return candidate
    return cast("WorkflowStatus", normalised)


def _phase_group(phase: Phase) -> PhaseGroup:
    return "python" if phase in {"python", "python_evaluation"} else "rust"


def parse_checklist(text: str) -> list[WorkItem]:
    """Parse every product item table, ignoring palette and metrics tables.

    Returns:
        Work items in checklist order.
    """
    items: list[WorkItem] = []
    header: tuple[str, ...] = ()
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("| Row | P |"):
            header = tuple(cell.strip() for cell in stripped.strip("|").split("|"))
            continue
        if not header:
            continue
        if not stripped.startswith("|"):
            header = ()
            continue
        cells = tuple(cell.strip() for cell in stripped.strip("|").split("|"))
        if len(cells) != len(header) or all(
            cell and set(cell) <= {"-", ":"} for cell in cells
        ):
            continue
        items.extend(_item_from_row(line, header, cells))
    return items


def _item_from_row(
    line: str, header: tuple[str, ...], cells: tuple[str, ...]
) -> list[WorkItem]:
    def column(name: str) -> str:
        return cells[header.index(name)] if name in header else ""

    priority = column("P")
    if priority not in {"P0", "P1"}:
        return []
    return [
        WorkItem(
            item_id=column("Row"),
            title=column("Row"),
            priority=priority,
            python_status=_status(column("Python Status") or column("Status")),
            python_evidence=column("Python Evidence") or column("Evidence"),
            rust_status=_status(column("Rust Status") or "NOT STARTED"),
            rust_evidence=column("Rust Evidence"),
            source_row=line,
        )
    ]


def _is_resolved(item: WorkItem, group: PhaseGroup) -> bool:
    status = item.python_status if group == "python" else item.rust_status
    evidence = item.python_evidence if group == "python" else item.rust_evidence
    if status == "WORKS" and evidence:
        return True
    return item.priority == "P1" and status == "DEFERRED" and bool(evidence)


def select_next_item(
    items: Sequence[WorkItem], state: WorkflowState
) -> WorkItem | None:
    """Resume the active item or choose the first open P0, then P1.

    Returns:
        The next work item, or None when only a gate remains.
    """
    if state.phase in EVALUATION_PHASES or state.phase == "complete":
        return None
    if state.active_item:
        for item in items:
            if item.item_id == state.active_item:
                return item
    group = _phase_group(state.phase)
    blocked = set(state.blocked)
    candidates = [
        item
        for item in items
        if item.item_id not in blocked and not _is_resolved(item, group)
    ]
    p0 = [item for item in candidates if item.priority == "P0"]
    p1 = [item for item in candidates if item.priority == "P1"]
    ordered = p0 + p1
    return ordered[0] if ordered else None


def readiness(items: Sequence[WorkItem], phase: Literal["python", "rust"]) -> list[str]:
    """Return every unmet checklist requirement for one phase."""
    errors: list[str] = []
    for item in items:
        status = item.python_status if phase == "python" else item.rust_status
        evidence = item.python_evidence if phase == "python" else item.rust_evidence
        if item.priority == "P0" and (status != "WORKS" or not evidence):
            errors.append(f"{item.item_id}: P0 not WORKS with evidence")
        if item.priority == "P1" and status not in {"WORKS", "DEFERRED"}:
            errors.append(f"{item.item_id}: P1 not WORKS or DEFERRED")
        if item.priority == "P1" and status == "DEFERRED" and not evidence:
            errors.append(f"{item.item_id}: DEFERRED without evidence")
    return errors


def evaluation_readiness(
    *,
    smoke: dict[str, object],
    comparison: dict[str, object] | None,
    baseline_avg_at_3: float | None,
    final_avg_at_3: float | None,
    phase: Literal["python", "rust"],
) -> list[str]:
    """Return every reason a matched evaluation artifact cannot open its gate."""
    errors: list[str] = []
    if not smoke.get("command") or not smoke.get("result"):
        errors.append("missing or incomplete smoke evidence")
    if comparison is None:
        errors.append("missing comparison data")
        return errors
    protocol = comparison.get("protocol")
    report = comparison.get("report")
    if not isinstance(protocol, dict) or not isinstance(report, dict):
        errors.append("malformed comparison artifact")
        return errors

    expected_agent = "langgraph" if phase == "python" else "niki-rust"
    if protocol.get("niki_agent") != expected_agent:
        errors.append(f"wrong niki_agent for {phase} gate")
    if protocol.get("benchmark_ref") != BENCHMARK_REF:
        errors.append("benchmark ref mismatch")
    if protocol.get("harbor_version") != HARBOR_VERSION:
        errors.append("harbor version mismatch")
    if protocol.get("retries") != 0 or protocol.get("n_concurrent_trials") != 1:
        errors.append("retry or concurrency mismatch")
    if protocol.get("trials_per_task") != TRIALS_PER_TASK:
        errors.append("trials per task mismatch")
    errors.extend(_task_errors(cast("dict[str, object]", report)))

    rate = report.get("niki_resolution_rate")
    if not isinstance(rate, (int, float)) or rate < MIN_RESOLUTION_RATE:
        errors.append("resolution rate below 55%")
    ci = report.get("difference_ci_95")
    if not isinstance(ci, (list, tuple)) or len(ci) != CI_BOUNDS:
        errors.append("missing paired CI")
    elif not isinstance(ci[0], (int, float)) or ci[0] < MIN_CI_LOWER:
        errors.append("CI lower bound below -10pp")

    if baseline_avg_at_3 is None or final_avg_at_3 is None:
        errors.append("missing unified avg@3 baseline or final")
    elif final_avg_at_3 < baseline_avg_at_3 - MAX_AVG_AT_3_DROP:
        errors.append("unified avg@3 more than 5pp below baseline")
    return errors


def _int_value(value: object, default: int) -> int:
    if isinstance(value, int) and not isinstance(value, bool):
        return value
    return default


def _task_errors(report: dict[str, object]) -> list[str]:
    outcomes = report.get("task_outcomes")
    if not isinstance(outcomes, list) or not outcomes:
        return ["missing task outcomes"]
    errors: list[str] = []
    niki_tasks: set[str] = set()
    claude_tasks: set[str] = set()
    for outcome in outcomes:
        if not isinstance(outcome, dict):
            errors.append("malformed task outcome")
            continue
        task_id = str(outcome.get("task_id", ""))
        for label, key, tasks in (
            ("niki", "niki_trials", niki_tasks),
            ("claude_code", "claude_code_trials", claude_tasks),
        ):
            trials = outcome.get(key)
            if not isinstance(trials, list) or len(trials) != TRIALS_PER_TASK:
                errors.append(f"{task_id}: {label} needs 3 attempts")
                continue
            indexes = sorted(
                _int_value(trial.get("attempt_index"), -1)
                for trial in trials
                if isinstance(trial, dict)
            )
            if indexes != list(range(1, TRIALS_PER_TASK + 1)):
                errors.append(f"{task_id}: {label} attempt indexes invalid")
            checksums = {
                str(trial.get("task_checksum", ""))
                for trial in trials
                if isinstance(trial, dict)
            }
            if len(checksums) != 1:
                errors.append(f"{task_id}: {label} checksum mismatch")
            tasks.add(task_id)
    if niki_tasks != claude_tasks:
        errors.append("task sets differ between agents")
    return errors


def _checklist_path() -> Path:
    override = os.environ.get(_CHECKLIST_ENV)
    return Path(override) if override else _REPO_ROOT / "docs" / "niki" / "CHECKLIST.md"


def _state_path() -> Path:
    override = os.environ.get(_STATE_ENV)
    return (
        Path(override)
        if override
        else _REPO_ROOT / "docs" / "niki" / "workflow-state.json"
    )


def _load_state(path: Path) -> WorkflowState:
    if not path.exists():
        return WorkflowState()
    return WorkflowState.from_dict(json.loads(path.read_text(encoding="utf-8")))


def _save_state(path: Path, state: WorkflowState) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(state.to_dict(), indent=2) + "\n", encoding="utf-8")


def _load_items() -> list[WorkItem]:
    return parse_checklist(_checklist_path().read_text(encoding="utf-8"))


def _print_errors(errors: Sequence[str]) -> None:
    for error in errors:
        print(error, file=sys.stderr)


def cmd_status(_args: argparse.Namespace) -> int:
    """Print the current phase, queue, blockers, and gate status.

    Returns:
        Zero after a successful status read.
    """
    items = _load_items()
    state = _load_state(_state_path())
    group = _phase_group(state.phase)
    p0 = [item for item in items if item.priority == "P0"]
    p1 = [item for item in items if item.priority == "P1"]
    print(f"phase: {state.phase}")
    print(f"active: {state.active_item} step={state.active_step}")
    print(f"owner: {state.owner} branch: {state.branch}")
    resolved = sum(1 for item in p0 if _is_resolved(item, group))
    print(f"{group}: {len(p0)} P0, {resolved} resolved")
    print(f"{group}: {len(p1)} P1")
    print(f"checklist gate: {'PASS' if not readiness(items, group) else 'OPEN'}")
    for item_id, reason in state.blocked.items():
        print(f"blocked: {item_id}: {reason}")
    return 0


def cmd_next(_args: argparse.Namespace) -> int:
    """Print the task card for the next eligible item without changing files.

    Returns:
        Zero after printing a card or the gate-only message.
    """
    items = _load_items()
    state = _load_state(_state_path())
    item = select_next_item(items, state)
    if item is None:
        print("No eligible item; only the phase gate remains.")
        return 0
    group = _phase_group(state.phase)
    status = item.python_status if group == "python" else item.rust_status
    evidence = item.python_evidence if group == "python" else item.rust_evidence
    step = state.item_steps.get(item.item_id, "")
    print(f"ITEM: {item.item_id}")
    print(f"TITLE: {item.title}")
    print(f"PRIORITY: {item.priority}")
    print(f"PHASE STATUS: {status}")
    print(f"EVIDENCE: {evidence}")
    print(f"SAVED STEP: {step}")
    print(f"SOURCE: {item.source_row}")
    return 0


def cmd_start(args: argparse.Namespace) -> int:
    """Claim one item for an owner and branch.

    Returns:
        Zero on success, or two for an invalid claim.
    """
    state = _load_state(_state_path())
    if state.active_item and state.active_item != args.id:
        print("Another item is active", file=sys.stderr)
        return 2
    if args.id in state.blocked:
        print("Item is blocked", file=sys.stderr)
        return 2
    if not any(item.item_id == args.id for item in _load_items()):
        print("Unknown item", file=sys.stderr)
        return 2
    state.active_item = args.id
    state.active_step = state.item_steps.get(args.id, "")
    state.owner = args.owner
    state.branch = args.branch
    state.item_steps.setdefault(args.id, "")
    _save_state(_state_path(), state)
    return 0


def cmd_step(args: argparse.Namespace) -> int:
    """Save the current step for the active item only.

    Returns:
        Zero on success, or two when the item is not active.
    """
    state = _load_state(_state_path())
    if state.active_item != args.id:
        print("Item is not active", file=sys.stderr)
        return 2
    state.active_step = args.name
    state.item_steps[args.id] = args.name
    _save_state(_state_path(), state)
    return 0


def cmd_block(args: argparse.Namespace) -> int:
    """Release the active claim and record a blocker without resolving the item.

    Returns:
        Zero on success, or two without a reason.
    """
    if not args.reason.strip():
        print("Block reason required", file=sys.stderr)
        return 2
    state = _load_state(_state_path())
    if state.active_item == args.id:
        state.active_item = None
        state.active_step = None
        state.owner = None
        state.branch = None
    state.blocked[args.id] = args.reason
    _save_state(_state_path(), state)
    return 0


def cmd_unblock(args: argparse.Namespace) -> int:
    """Clear a blocker after a resolution note; the item still needs a retest.

    Returns:
        Zero on success, or two without a resolution note.
    """
    if not args.resolution.strip():
        print("Resolution required", file=sys.stderr)
        return 2
    state = _load_state(_state_path())
    state.blocked.pop(args.id, None)
    state.item_steps.setdefault(args.id, args.resolution)
    _save_state(_state_path(), state)
    return 0


def cmd_finish(args: argparse.Namespace) -> int:
    """Record verified evidence for a WORKS item in the active implementation phase.

    Returns:
        Zero on success, or two for invalid evidence or commit.
    """
    state = _load_state(_state_path())
    if state.phase not in IMPLEMENTATION_PHASES:
        print("Finish requires an implementation phase", file=sys.stderr)
        return 2
    item = next((i for i in _load_items() if i.item_id == args.id), None)
    if item is None:
        print("Unknown item", file=sys.stderr)
        return 2
    group = _phase_group(state.phase)
    status = item.python_status if group == "python" else item.rust_status
    evidence = item.python_evidence if group == "python" else item.rust_evidence
    if status != "WORKS" or not evidence:
        print("Item not WORKS with evidence", file=sys.stderr)
        return 2
    if len(args.commit) != COMMIT_SHA_LENGTH or any(
        c not in "0123456789abcdef" for c in args.commit.lower()
    ):
        print("Full 40-character commit SHA required", file=sys.stderr)
        return 2
    try:
        subprocess.run(
            ["git", "cat-file", "-e", f"{args.commit}^{{commit}}"],
            check=True,
            capture_output=True,
            cwd=_REPO_ROOT,
        )
    except subprocess.CalledProcessError:
        print("Commit does not resolve", file=sys.stderr)
        return 2
    if not args.evidence.strip():
        print("Evidence required", file=sys.stderr)
        return 2
    state.completed[args.id] = {"commit": args.commit, "evidence": args.evidence}
    state.last_commit = args.commit
    if state.active_item == args.id:
        state.active_item = None
        state.active_step = None
        state.owner = None
        state.branch = None
    _save_state(_state_path(), state)
    return 0


def cmd_readiness(args: argparse.Namespace) -> int:
    """Check that every P0 and classified P1 has evidence for one phase.

    Returns:
        Zero when the gate passes, or one when it fails.
    """
    items = _load_items()
    state = _load_state(_state_path())
    errors = readiness(items, args.phase)
    errors.extend(
        f"{item_id}: blocked ({reason})" for item_id, reason in state.blocked.items()
    )
    if errors:
        _print_errors(errors)
        return 1
    return 0


def cmd_advance(args: argparse.Namespace) -> int:
    """Advance one phase only after its preceding gate passes.

    Returns:
        Zero on advance, one for a failed gate, or two for an invalid transition.
    """
    state = _load_state(_state_path())
    if args.to not in PHASES:
        print("Invalid phase", file=sys.stderr)
        return 2
    if PHASES.index(args.to) != PHASES.index(state.phase) + 1:
        print("Invalid phase transition", file=sys.stderr)
        return 2
    if state.active_item is not None:
        print("Active item must be cleared first", file=sys.stderr)
        return 2

    if args.to in {"python_evaluation", "rust_evaluation"}:
        errors = readiness(_load_items(), _phase_group(state.phase))
        if errors:
            _print_errors(errors)
            return 1
    else:
        gate = "python_evaluation" if args.to == "rust" else "rust_evaluation"
        last_eval = state.last_eval or {}
        if last_eval.get(gate) != "passed":
            print(f"{gate} gate has not passed", file=sys.stderr)
            return 1
    state.phase = args.to
    _save_state(_state_path(), state)
    return 0


def cmd_reopen(args: argparse.Namespace) -> int:
    """Return to an implementation phase after a failed evaluation."""
    state = _load_state(_state_path())
    if args.to not in IMPLEMENTATION_PHASES:
        print("Invalid reopen target", file=sys.stderr)
        return 2
    if state.phase not in {*EVALUATION_PHASES, "complete"}:
        print("No evaluation to reopen", file=sys.stderr)
        return 2
    if not args.reason.strip():
        print("Reopen reason required", file=sys.stderr)
        return 2
    gate = "python_evaluation" if args.to == "python" else "rust_evaluation"
    last_eval = dict(state.last_eval or {})
    last_eval[gate] = "reopened"
    last_eval["reason"] = args.reason
    state.last_eval = last_eval
    state.phase = args.to
    _save_state(_state_path(), state)
    return 0


def cmd_evaluation_readiness(args: argparse.Namespace) -> int:
    """Check the matched evaluation artifacts and record the gate result.

    Returns:
        Zero when the gate passes, one when it fails, or two for bad artifacts.
    """
    smoke: dict[str, object] = {}
    comparison: dict[str, object] | None = None
    baseline: float | None = None
    final: float | None = None
    try:
        if args.smoke:
            smoke = json.loads(Path(args.smoke).read_text(encoding="utf-8"))
        if args.comparison:
            comparison = json.loads(Path(args.comparison).read_text(encoding="utf-8"))
        baseline = float(args.baseline) if args.baseline is not None else None
        final = float(args.final) if args.final is not None else None
    except (OSError, ValueError, TypeError) as exc:
        print(f"Invalid evaluation artifact: {exc}", file=sys.stderr)
        return 2
    errors = evaluation_readiness(
        smoke=smoke,
        comparison=comparison,
        baseline_avg_at_3=baseline,
        final_avg_at_3=final,
        phase=args.phase,
    )
    state = _load_state(_state_path())
    last_eval = dict(state.last_eval or {})
    last_eval[f"{args.phase}_evaluation"] = "passed" if not errors else "failed"
    last_eval["baseline_avg_at_3"] = baseline
    last_eval["final_avg_at_3"] = final
    state.last_eval = last_eval
    _save_state(_state_path(), state)
    if errors:
        _print_errors(errors)
        return 1
    return 0


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("status")
    sub.add_parser("next")

    claim = sub.add_parser("start")
    claim.add_argument("--id", required=True)
    claim.add_argument("--owner", required=True)
    claim.add_argument("--branch", required=True)

    step = sub.add_parser("step")
    step.add_argument("--id", required=True)
    step.add_argument("--name", required=True)

    block = sub.add_parser("block")
    block.add_argument("--id", required=True)
    block.add_argument("--reason", required=True)

    unblock = sub.add_parser("unblock")
    unblock.add_argument("--id", required=True)
    unblock.add_argument("--resolution", required=True)

    finish = sub.add_parser("finish")
    finish.add_argument("--id", required=True)
    finish.add_argument("--commit", required=True)
    finish.add_argument("--evidence", required=True)

    gate = sub.add_parser("readiness")
    gate.add_argument("--phase", required=True, choices=["python", "rust"])

    advance = sub.add_parser("advance")
    advance.add_argument("--to", required=True)

    reopen = sub.add_parser("reopen")
    reopen.add_argument("--to", required=True)
    reopen.add_argument("--reason", required=True)

    evaluation = sub.add_parser("evaluation-readiness")
    evaluation.add_argument("--phase", required=True, choices=["python", "rust"])
    evaluation.add_argument("--smoke")
    evaluation.add_argument("--comparison")
    evaluation.add_argument("--baseline")
    evaluation.add_argument("--final")
    return parser


def main() -> None:
    """Dispatch a workflow command and exit with its gate status."""
    args = _build_parser().parse_args()
    handlers = {
        "status": cmd_status,
        "next": cmd_next,
        "start": cmd_start,
        "step": cmd_step,
        "block": cmd_block,
        "unblock": cmd_unblock,
        "finish": cmd_finish,
        "readiness": cmd_readiness,
        "advance": cmd_advance,
        "reopen": cmd_reopen,
        "evaluation-readiness": cmd_evaluation_readiness,
    }
    sys.exit(handlers[args.command](args))


if __name__ == "__main__":
    main()
