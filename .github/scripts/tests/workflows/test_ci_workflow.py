"""CI workflow: one package, one check job, one aggregate gate.

The upstream workflow ran a matrix of package jobs, release-please gates, and a
curated-apply job. After the independence work there is one product package, so
these tests pin the small shape that is left instead of the machinery that left
with it.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import yaml

ROOT = Path(__file__).resolve().parents[4]
CI_WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"


def _load() -> dict[str, Any]:
    return yaml.safe_load(CI_WORKFLOW.read_text())


def _steps(job: str) -> list[dict[str, Any]]:
    return _load()["jobs"][job]["steps"]


def _by_name(job: str) -> dict[str, dict[str, Any]]:
    return {step.get("name", ""): step for step in _steps(job)}


def test_ci_runs_the_product_and_rust_jobs_plus_an_aggregate() -> None:
    """A missing job means `CI Success` can never be satisfied."""
    jobs = _load()["jobs"]
    assert set(jobs) == {"check", "rust", "ci_success"}


def test_check_job_runs_make_check_in_the_product_package() -> None:
    """The authoritative gate is `make check` as a user would run it."""
    step = _by_name("check")["✅ make check"]
    assert step["working-directory"] == "libs/code"
    run = step["run"]
    assert "make check" in run
    assert 'PYTEST_EXTRA="-n 4"' in run, "xdist must stay capped on runners"


def test_check_job_unsets_no_color() -> None:
    """`NO_COLOR` breaks the doctor snapshot that asserts links render."""
    run = _by_name("check")["✅ make check"]["run"]
    assert "unset NO_COLOR" in run


def test_success_gate_fails_on_any_result_but_success() -> None:
    """The aggregate must not pass on `cancelled`, `skipped`, or `failure`."""
    job = _load()["jobs"]["ci_success"]
    assert job["needs"] == ["check", "rust"]
    assert job["if"] == "always()"
    run = job["steps"][0]["run"]
    assert '"$CHECK_RESULT" != "success"' in run
    assert '"$RUST_RESULT" != "success"' in run


def test_no_job_targets_a_removed_package() -> None:
    """The workflow names one package; a window into the archive would fail."""
    text = CI_WORKFLOW.read_text()
    for removed in (
        "libs/evals",
        "libs/deepagents",
        "libs/acp",
        "libs/talon",
        "libs/partners",
    ):
        assert removed not in text, f"{removed} is archived, not built"
