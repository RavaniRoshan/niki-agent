"""Tests for the dynamic Niki completion workflow controller."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from dataclasses import replace
from pathlib import Path

import pytest

from scripts.niki_workflow import (
    WorkflowState,
    evaluation_readiness,
    parse_checklist,
    readiness,
    select_next_item,
)

REPO_ROOT = Path(__file__).resolve().parents[4]
FIXTURES = Path(__file__).resolve().parents[2] / "tests" / "fixtures" / "niki-workflow"
SCRIPT = REPO_ROOT / "libs" / "code" / "scripts" / "niki_workflow.py"
ITEM_IDS = {"P0-FIRST", "P0-SECOND", "P0-BLOCKED", "RUST-P0"}


def _items() -> list:
    return parse_checklist((FIXTURES / "checklist.md").read_text(encoding="utf-8"))


@pytest.fixture
def workspace(tmp_path):
    """Create an isolated checklist/state pair for CLI tests."""

    def build(state_name: str = "state.json") -> tuple[Path, Path]:
        checklist = tmp_path / "checklist.md"
        checklist.write_text((FIXTURES / "checklist.md").read_text(encoding="utf-8"))
        state = tmp_path / state_name
        state.write_text((FIXTURES / state_name).read_text(encoding="utf-8"))
        return checklist, state

    return build


def run_cli(
    checklist: Path, state: Path, *arguments: str
) -> subprocess.CompletedProcess[str]:
    env = dict(os.environ)
    env["NIKI_WORKFLOW_CHECKLIST"] = str(checklist)
    env["NIKI_WORKFLOW_STATE"] = str(state)
    return subprocess.run(
        [sys.executable, str(SCRIPT), *arguments],
        cwd=REPO_ROOT / "libs" / "code",
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )


def set_row(
    checklist: Path, item_id: str, group: str, status: str, evidence: str
) -> None:
    lines = checklist.read_text(encoding="utf-8").splitlines()
    for index, line in enumerate(lines):
        if not line.startswith(f"| {item_id} |"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        offset = 2 if group == "python" else 4
        cells[offset] = status
        cells[offset + 1] = evidence
        lines[index] = "| " + " | ".join(cells) + " |"
    checklist.write_text("\n".join(lines) + "\n", encoding="utf-8")


def make_python_ready(checklist: Path) -> None:
    for item_id in ITEM_IDS:
        set_row(checklist, item_id, "python", "WORKS", "tests/test_ok.py::test_ok")
    set_row(checklist, "P1-OPEN", "python", "DEFERRED", "owner decision recorded")


def valid_comparison(agent: str = "langgraph", rate: float = 0.6) -> dict[str, object]:
    trials = [
        {
            "task_id": "t1",
            "task_checksum": "checksum",
            "attempt_index": index,
            "reward": 1.0,
        }
        for index in range(1, 4)
    ]
    return {
        "protocol": {
            "niki_agent": agent,
            "benchmark_ref": "terminal-bench/terminal-bench@4.1.0",
            "harbor_version": "0.25.0",
            "retries": 0,
            "n_concurrent_trials": 1,
            "trials_per_task": 3,
        },
        "report": {
            "niki_resolution_rate": rate,
            "difference_ci_95": [-0.05, 0.10],
            "task_outcomes": [
                {"task_id": "t1", "niki_trials": trials, "claude_code_trials": trials}
            ],
        },
    }


def write_json(path: Path, payload: object) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def test_parse_checklist_preserves_phase_status_and_evidence():
    items = _items()
    assert len(items) == 7
    by_id = {item.item_id: item for item in items}
    assert by_id["P0-WORKS"].python_status == "WORKS"
    assert by_id["P0-WORKS"].python_evidence == "tests/test_ok.py::test_ok"
    assert by_id["P0-WORKS"].rust_status == "NOT STARTED"
    assert by_id["P0-WORKS"].rust_evidence == ""
    assert by_id["P1-DEFERRED"].python_status == "DEFERRED"
    assert by_id["P0-BLOCKED"].python_status == "PARTIAL"


def test_next_selects_first_open_p0_before_p1():
    items = _items()
    assert select_next_item(items, WorkflowState()).item_id == "P0-FIRST"
    resolved = [
        replace(
            item,
            python_status="WORKS",
            python_evidence="tests/test_ok.py::test_ok",
        )
        if item.item_id in ITEM_IDS
        else item
        for item in items
    ]
    assert select_next_item(resolved, WorkflowState()).item_id == "P1-OPEN"


def test_next_resumes_active_item_without_claiming_another(workspace):
    items = _items()
    state = WorkflowState(
        active_item="P0-SECOND",
        active_step="write regression test",
        item_steps={"P0-SECOND": "write regression test"},
    )
    assert select_next_item(items, state).item_id == "P0-SECOND"
    checklist, state_path = workspace("selector-state.json")
    state_path.write_text(json.dumps(state.to_dict()), encoding="utf-8")
    result = run_cli(checklist, state_path, "next")
    assert result.returncode == 0
    assert "ITEM: P0-SECOND" in result.stdout
    assert "SAVED STEP: write regression test" in result.stdout


def test_step_updates_only_active_item(workspace):
    checklist, state = workspace("selector-state.json")
    assert (
        run_cli(
            checklist,
            state,
            "start",
            "--id",
            "P0-FIRST",
            "--owner",
            "agent",
            "--branch",
            "main",
        ).returncode
        == 0
    )
    assert (
        run_cli(
            checklist, state, "step", "--id", "P0-FIRST", "--name", "probe"
        ).returncode
        == 0
    )
    denied = run_cli(checklist, state, "step", "--id", "P0-SECOND", "--name", "illegal")
    assert denied.returncode == 2
    saved = json.loads(state.read_text(encoding="utf-8"))
    assert saved["item_steps"] == {"P0-FIRST": "probe"}
    assert saved["active_step"] == "probe"


def test_blocked_p0_remains_open_until_evidence_is_recorded(workspace):
    checklist, state = workspace("selector-state.json")
    run_cli(
        checklist,
        state,
        "start",
        "--id",
        "P0-FIRST",
        "--owner",
        "agent",
        "--branch",
        "main",
    )
    run_cli(checklist, state, "step", "--id", "P0-FIRST", "--name", "probe")
    assert (
        run_cli(
            checklist, state, "block", "--id", "P0-FIRST", "--reason", "owner evidence"
        ).returncode
        == 0
    )
    following = run_cli(checklist, state, "next")
    assert "ITEM: P0-SECOND" in following.stdout
    assert "P0-FIRST" not in following.stdout
    assert run_cli(checklist, state, "readiness", "--phase", "python").returncode == 1
    assert (
        run_cli(
            checklist,
            state,
            "start",
            "--id",
            "P0-FIRST",
            "--owner",
            "agent",
            "--branch",
            "main",
        ).returncode
        == 2
    )
    assert run_cli(checklist, state, "readiness", "--phase", "python").returncode == 1
    assert (
        run_cli(
            checklist,
            state,
            "unblock",
            "--id",
            "P0-FIRST",
            "--resolution",
            "owner checked",
        ).returncode
        == 0
    )
    resumed = run_cli(checklist, state, "next")
    assert "ITEM: P0-FIRST" in resumed.stdout
    assert "SAVED STEP: probe" in resumed.stdout


def test_unblock_requires_resolution_and_keeps_item_unresolved_until_retest(workspace):
    checklist, state = workspace("selector-state.json")
    assert (
        run_cli(
            checklist, state, "unblock", "--id", "P0-FIRST", "--resolution", "  "
        ).returncode
        == 2
    )
    assert (
        run_cli(
            checklist, state, "block", "--id", "P0-FIRST", "--reason", "waiting"
        ).returncode
        == 0
    )
    assert (
        run_cli(
            checklist,
            state,
            "unblock",
            "--id",
            "P0-FIRST",
            "--resolution",
            "retry planned",
        ).returncode
        == 0
    )
    saved = json.loads(state.read_text(encoding="utf-8"))
    assert "P0-FIRST" not in saved["blocked"]
    assert "P0-FIRST" in saved["item_steps"]
    assert select_next_item(_items(), WorkflowState()).item_id == "P0-FIRST"
    assert run_cli(checklist, state, "readiness", "--phase", "python").returncode == 1


def test_workflow_allows_only_one_active_item(workspace):
    checklist, state = workspace("selector-state.json")
    assert (
        run_cli(
            checklist,
            state,
            "start",
            "--id",
            "P0-FIRST",
            "--owner",
            "agent",
            "--branch",
            "main",
        ).returncode
        == 0
    )
    second = run_cli(
        checklist,
        state,
        "start",
        "--id",
        "P0-SECOND",
        "--owner",
        "agent",
        "--branch",
        "main",
    )
    assert second.returncode == 2
    status = run_cli(checklist, state, "status")
    assert "active: P0-FIRST" in status.stdout


def test_finish_requires_works_commit_and_evidence(workspace):
    checklist, state = workspace("selector-state.json")
    sha = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()

    def finish(*arguments: str) -> subprocess.CompletedProcess[str]:
        return run_cli(checklist, state, "finish", "--id", "P0-WORKS", *arguments)

    assert finish("--commit", sha[:8], "--evidence", "test").returncode == 2
    assert finish("--commit", "0" * 40, "--evidence", "test").returncode == 2
    assert finish("--commit", sha, "--evidence", "  ").returncode == 2
    assert (
        finish(
            "--commit", sha, "--evidence", "tests/test_ok.py::test_ok passed"
        ).returncode
        == 0
    )
    saved = json.loads(state.read_text(encoding="utf-8"))
    assert saved["completed"]["P0-WORKS"]["commit"] == sha


def test_readiness_requires_evidence_for_every_p0(workspace):
    errors = readiness(_items(), "python")
    assert any("P0-FIRST" in error for error in errors)
    assert any("P0-SECOND" in error for error in errors)
    checklist, state = workspace("selector-state.json")
    assert run_cli(checklist, state, "readiness", "--phase", "python").returncode == 1
    make_python_ready(checklist)
    assert run_cli(checklist, state, "readiness", "--phase", "python").returncode == 0


def test_readiness_requires_p1_work_or_approved_deferral():
    items = _items()
    open_error = readiness(items, "python")
    assert any("P1-OPEN" in error for error in open_error)
    deferred = [
        replace(
            item, python_status="DEFERRED", python_evidence="owner decision recorded"
        )
        if item.item_id == "P1-OPEN"
        else item
        for item in items
    ]
    assert not any("P1-OPEN" in error for error in readiness(deferred, "python"))
    unrecorded = [
        replace(item, python_status="DEFERRED", python_evidence="")
        if item.item_id == "P1-OPEN"
        else item
        for item in items
    ]
    assert any(
        "DEFERRED without evidence" in error
        for error in readiness(unrecorded, "python")
    )


def test_evaluation_gate_rejects_missing_or_incomplete_artifacts():
    errors = evaluation_readiness(
        smoke={},
        comparison=None,
        baseline_avg_at_3=None,
        final_avg_at_3=None,
        phase="python",
    )
    assert any("smoke" in error for error in errors)
    assert any("comparison" in error for error in errors)
    broken = valid_comparison(rate=0.4)
    errors = evaluation_readiness(
        smoke={"command": "niki -n", "result": "passed"},
        comparison=broken,
        baseline_avg_at_3=0.7,
        final_avg_at_3=0.68,
        phase="python",
    )
    assert any("55%" in error for error in errors)
    wrong_agent = valid_comparison(agent="niki-rust")
    errors = evaluation_readiness(
        smoke={"command": "niki -n", "result": "passed"},
        comparison=wrong_agent,
        baseline_avg_at_3=0.7,
        final_avg_at_3=0.68,
        phase="python",
    )
    assert any("niki_agent" in error for error in errors)


def test_rust_phase_requires_python_readiness_gate(workspace):
    checklist, state = workspace("selector-state.json")
    assert (
        run_cli(checklist, state, "advance", "--to", "python_evaluation").returncode
        == 1
    )
    make_python_ready(checklist)
    assert (
        run_cli(checklist, state, "advance", "--to", "python_evaluation").returncode
        == 0
    )
    assert run_cli(checklist, state, "advance", "--to", "rust").returncode == 1
    smoke = write_json(
        checklist.parent / "smoke.json", {"command": "niki -n", "result": "passed"}
    )
    comparison_file = write_json(
        checklist.parent / "comparison.json", valid_comparison()
    )
    result = run_cli(
        checklist,
        state,
        "evaluation-readiness",
        "--phase",
        "python",
        "--smoke",
        str(smoke),
        "--comparison",
        str(comparison_file),
        "--baseline",
        "0.70",
        "--final",
        "0.68",
    )
    assert result.returncode == 0, result.stderr
    assert run_cli(checklist, state, "advance", "--to", "rust").returncode == 0
    saved = json.loads(state.read_text(encoding="utf-8"))
    assert saved["phase"] == "rust"
    assert saved["last_eval"]["python_evaluation"] == "passed"
    assert smoke.exists()


def test_failed_evaluation_reopens_implementation_phase(workspace):
    checklist, state = workspace("selector-state.json")
    make_python_ready(checklist)
    assert (
        run_cli(checklist, state, "advance", "--to", "python_evaluation").returncode
        == 0
    )
    smoke = write_json(
        checklist.parent / "smoke.json", {"command": "niki -n", "result": "passed"}
    )
    comparison_file = write_json(
        checklist.parent / "comparison.json", valid_comparison(rate=0.4)
    )
    failed = run_cli(
        checklist,
        state,
        "evaluation-readiness",
        "--phase",
        "python",
        "--smoke",
        str(smoke),
        "--comparison",
        str(comparison_file),
        "--baseline",
        "0.70",
        "--final",
        "0.68",
    )
    assert failed.returncode == 1
    assert run_cli(checklist, state, "advance", "--to", "rust").returncode == 1
    assert (
        run_cli(
            checklist, state, "reopen", "--to", "python", "--reason", "below target"
        ).returncode
        == 0
    )
    saved = json.loads(state.read_text(encoding="utf-8"))
    assert saved["phase"] == "python"
    assert saved["last_eval"]["reason"] == "below target"
