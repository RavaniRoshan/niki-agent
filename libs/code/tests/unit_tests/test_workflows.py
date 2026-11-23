"""Tests for staged workflows."""

from __future__ import annotations

import pytest

from deepagents_code.application.workflows import (
    InMemoryWorkflowRepository,
    WorkflowRunner,
)
from deepagents_code.domain.workflow import (
    WorkflowDefinition,
    WorkflowStep,
)


@pytest.mark.asyncio
async def test_workflow_runs_stages_in_dependency_order():
    defn = WorkflowDefinition(
        name="w",
        version=1,
        steps=(
            WorkflowStep(step_id="a", instruction="do a"),
            WorkflowStep(step_id="b", instruction="do b", depends_on=("a",)),
        ),
    )
    runner = WorkflowRunner()
    out = [e async for e in runner.run(defn, "s1")]
    # a before b
    a_idx = next(i for i, e in enumerate(out) if e.payload.get("step_id") == "a")
    b_idx = next(i for i, e in enumerate(out) if e.payload.get("step_id") == "b")
    assert a_idx < b_idx


@pytest.mark.asyncio
async def test_workflow_runs_independent_steps_in_parallel():
    defn = WorkflowDefinition(
        name="w",
        version=1,
        steps=(
            WorkflowStep(step_id="a", instruction="do a"),
            WorkflowStep(step_id="b", instruction="do b"),
        ),
    )
    runner = WorkflowRunner()
    out = [e async for e in runner.run(defn, "s1")]
    assert len([e for e in out if e.payload.get("step_id") in ("a", "b")]) == 2


@pytest.mark.asyncio
async def test_workflow_aggregates_step_results():
    defn = WorkflowDefinition(
        name="w",
        version=1,
        steps=(WorkflowStep(step_id="a", instruction="do a"),),
    )
    runner = WorkflowRunner()
    out = [e async for e in runner.run(defn, "s1")]
    step_events = [e for e in out if e.payload.get("step_id") == "a"]
    assert step_events and "result" in step_events[0].payload


@pytest.mark.asyncio
async def test_workflow_resume_skips_checkpointed_steps():
    defn = WorkflowDefinition(
        name="w",
        version=1,
        steps=(
            WorkflowStep(step_id="a", instruction="do a"),
            WorkflowStep(step_id="b", instruction="do b", depends_on=("a",)),
        ),
    )
    runner = WorkflowRunner()
    out = [e async for e in runner.run(defn, "s1", completed_steps=("a",))]
    step_ids = [e.payload.get("step_id") for e in out if "step_id" in e.payload]
    assert "a" not in step_ids
    assert "b" in step_ids


@pytest.mark.asyncio
async def test_workflow_rejects_cycle_before_start():
    with pytest.raises(ValueError):
        WorkflowDefinition(
            name="w",
            version=1,
            steps=(
                WorkflowStep(step_id="a", instruction="x", depends_on=("b",)),
                WorkflowStep(step_id="b", instruction="y", depends_on=("a",)),
            ),
        )


def test_repository_load_save():
    repo = InMemoryWorkflowRepository()
    defn = WorkflowDefinition(name="w", version=1, steps=(WorkflowStep("a", "x"),))
    repo.save(defn)
    assert repo.load("w") == defn
    assert repo.load("missing") is None
