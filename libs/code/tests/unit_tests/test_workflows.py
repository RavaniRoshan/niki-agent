"""Tests for staged workflows."""

from __future__ import annotations

import asyncio
from types import MethodType

import pytest

from deepagents_code.application.workflows import (
    InMemoryWorkflowRepository,
    WorkflowRunner,
)
from deepagents_code.domain.workflow import (
    WorkflowDefinition,
    WorkflowRun,
    WorkflowStep,
)


class EchoStepPort:
    async def run_step(
        self, step: WorkflowStep, *, session_id: str, workflow_name: str
    ) -> str:
        _ = session_id, workflow_name
        return f"result of {step.step_id}: {step.instruction}"


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
    runner = WorkflowRunner(step_port=EchoStepPort())
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
    entered = 0
    both_entered = asyncio.Event()

    class ParallelStepPort:
        async def run_step(
            self, step: WorkflowStep, *, session_id: str, workflow_name: str
        ) -> str:
            _ = session_id, workflow_name
            nonlocal entered
            entered += 1
            if entered == 2:
                both_entered.set()
            await both_entered.wait()
            return step.step_id

    runner = WorkflowRunner(step_port=ParallelStepPort())
    out = [e async for e in runner.run(defn, "s1")]
    assert len([e for e in out if e.kind.value == "task_finished"]) == 2


@pytest.mark.asyncio
async def test_workflow_aggregates_step_results():
    defn = WorkflowDefinition(
        name="w",
        version=1,
        steps=(WorkflowStep(step_id="a", instruction="do a"),),
    )
    runner = WorkflowRunner(step_port=EchoStepPort())
    out = [e async for e in runner.run(defn, "s1")]
    step_events = [
        e
        for e in out
        if e.kind.value == "task_finished" and e.payload.get("step_id") == "a"
    ]
    assert step_events
    assert step_events[0].payload["result"] == "result of a: do a"


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
    runner = WorkflowRunner(step_port=EchoStepPort())
    out = [
        e
        async for e in runner.run(
            defn,
            "s1",
            run=WorkflowRun("w", "s1", ("a",), (("a", "result of a: do a"),)),
        )
    ]
    step_ids = [e.payload.get("step_id") for e in out if "step_id" in e.payload]
    assert "a" not in step_ids
    assert "b" in step_ids


def test_workflow_rejects_cycle_before_start():
    with pytest.raises(ValueError, match="cycle"):
        WorkflowDefinition(
            name="w",
            version=1,
            steps=(
                WorkflowStep(step_id="a", instruction="x", depends_on=("b",)),
                WorkflowStep(step_id="b", instruction="y", depends_on=("a",)),
            ),
        )


@pytest.mark.parametrize(
    "kwargs",
    [
        {"name": " ", "version": 1, "steps": (WorkflowStep("a", "x"),)},
        {"name": "../escape", "version": 1, "steps": (WorkflowStep("a", "x"),)},
        {"name": "w", "version": 0, "steps": (WorkflowStep("a", "x"),)},
        {"name": "w", "version": 1, "steps": ()},
        {
            "name": "w",
            "version": 1,
            "steps": (WorkflowStep("a", "x", depends_on=("a",)),),
        },
    ],
)
def test_workflow_rejects_invalid_identity_and_dependencies(kwargs):
    with pytest.raises(ValueError, match="workflow"):
        WorkflowDefinition(**kwargs)


def test_workflow_run_rejects_invalid_event_sequence():
    with pytest.raises(ValueError, match="event sequence"):
        WorkflowRun("verify", "session-1", event_sequence=-1)


@pytest.mark.asyncio
async def test_runner_executes_steps_through_injected_port_and_emits_lifecycle():
    class StepPort:
        def __init__(self) -> None:
            self.calls = []

        async def run_step(
            self, step: WorkflowStep, *, session_id: str, workflow_name: str
        ) -> str:
            self.calls.append((step.step_id, session_id, workflow_name))
            return f"result:{step.step_id}"

    definition = WorkflowDefinition(
        name="verify",
        version=2,
        steps=(WorkflowStep("inspect", "inspect"),),
    )
    port = StepPort()
    runner = WorkflowRunner(step_port=port)

    events = [event async for event in runner.run(definition, "session-1")]

    assert port.calls == [("inspect", "session-1", "verify")]
    assert [event.kind.value for event in events] == [
        "workflow_started",
        "task_started",
        "task_finished",
        "workflow_finished",
    ]
    assert events[-1].payload == {
        "workflow_name": "verify",
        "version": 2,
        "event_sequence": 4,
        "completed_steps": ["inspect"],
        "results": {"inspect": "result:inspect"},
    }
    assert [event.sequence for event in events] == [1, 2, 3, 4]


@pytest.mark.asyncio
async def test_runner_resumes_from_checkpointed_workflow_run():
    class StepPort:
        def __init__(self) -> None:
            self.calls = []

        async def run_step(
            self, step: WorkflowStep, *, session_id: str, workflow_name: str
        ) -> str:
            _ = session_id, workflow_name
            self.calls.append(step.step_id)
            return f"result:{step.step_id}"

    definition = WorkflowDefinition(
        name="verify",
        version=1,
        steps=(
            WorkflowStep("inspect", "inspect"),
            WorkflowStep("repair", "repair", depends_on=("inspect",)),
        ),
    )
    port = StepPort()
    run = WorkflowRun(
        "verify",
        "session-1",
        ("inspect",),
        (("inspect", "result:inspect"),),
        event_sequence=10,
    )
    events = [
        event
        async for event in WorkflowRunner(step_port=port).run(
            definition, "session-1", run=run
        )
    ]

    assert port.calls == ["repair"]
    assert events[-1].payload["completed_steps"] == ["inspect", "repair"]
    assert events[-1].payload["results"] == {
        "inspect": "result:inspect",
        "repair": "result:repair",
    }
    assert [event.sequence for event in events] == [11, 12, 13, 14]
    assert events[-1].payload["event_sequence"] == 14


@pytest.mark.asyncio
async def test_runner_reports_step_failure_and_does_not_run_dependents():
    class StepPort:
        def __init__(self) -> None:
            self.calls = []

        async def run_step(
            self, step: WorkflowStep, *, session_id: str, workflow_name: str
        ) -> str:
            _ = session_id, workflow_name
            self.calls.append(step.step_id)
            if step.step_id == "inspect":
                msg = "provider unavailable"
                raise RuntimeError(msg)
            return "unexpected"

    definition = WorkflowDefinition(
        name="verify",
        version=1,
        steps=(
            WorkflowStep("inspect", "inspect"),
            WorkflowStep("repair", "repair", depends_on=("inspect",)),
        ),
    )
    port = StepPort()
    events = [
        event
        async for event in WorkflowRunner(step_port=port).run(definition, "session-1")
    ]

    assert port.calls == ["inspect"]
    assert [event.kind.value for event in events][-2:] == [
        "task_failed",
        "workflow_failed",
    ]
    assert events[-1].payload["failed_step"] == "inspect"


@pytest.mark.asyncio
async def test_runner_cancels_independent_sibling_after_stage_failure():
    sibling_started = asyncio.Event()
    sibling_cancelled = asyncio.Event()

    class StepPort:
        async def run_step(
            self, step: WorkflowStep, *, session_id: str, workflow_name: str
        ) -> str:
            _ = session_id, workflow_name
            if step.step_id == "slow":
                sibling_started.set()
                try:
                    await asyncio.Future()
                except asyncio.CancelledError:
                    sibling_cancelled.set()
                    raise
            await sibling_started.wait()
            msg = "provider unavailable"
            raise RuntimeError(msg)

    definition = WorkflowDefinition(
        "verify",
        1,
        (WorkflowStep("fail", "fail"), WorkflowStep("slow", "slow")),
    )
    events = [
        event
        async for event in WorkflowRunner(step_port=StepPort()).run(
            definition, "session-1"
        )
    ]

    assert sibling_cancelled.is_set()
    assert events[-1].kind.value == "workflow_failed"


@pytest.mark.asyncio
async def test_closing_workflow_stream_cancels_active_step():
    started = asyncio.Event()
    cancelled = asyncio.Event()

    class WaitingStepPort:
        async def run_step(
            self, step: WorkflowStep, *, session_id: str, workflow_name: str
        ) -> str:
            _ = step, session_id, workflow_name
            started.set()
            try:
                await asyncio.Future()
            except asyncio.CancelledError:
                cancelled.set()
                raise
            msg = "waiting step returned unexpectedly"
            raise AssertionError(msg)

    definition = WorkflowDefinition("verify", 1, (WorkflowStep("inspect", "x"),))
    events = WorkflowRunner(step_port=WaitingStepPort()).run(definition, "session-1")
    assert (await anext(events)).kind.value == "workflow_started"
    assert (await anext(events)).kind.value == "task_started"
    await started.wait()

    await events.aclose()

    assert cancelled.is_set()


def test_profile_workflow_repository_persists_definitions(tmp_path, monkeypatch):
    from deepagents_code.application.workflows import ProfileWorkflowRepository

    monkeypatch.setattr(
        "deepagents_code.sessions.get_workflows_dir", lambda: tmp_path / "workflows"
    )
    definition = WorkflowDefinition(
        name="verify",
        version=1,
        steps=(WorkflowStep("inspect", "inspect"),),
    )

    ProfileWorkflowRepository().save(definition)

    assert ProfileWorkflowRepository().load("verify") == definition
    assert list((tmp_path / "workflows").iterdir()) == [
        tmp_path / "workflows" / "verify.json"
    ]


def test_profile_workflow_repository_rejects_symlinked_definition(
    tmp_path, monkeypatch
):
    from deepagents_code.application.workflows import ProfileWorkflowRepository

    workflow_dir = tmp_path / "workflows"
    workflow_dir.mkdir()
    outside = tmp_path / "outside.json"
    outside.write_text('{"name":"verify","version":1,"steps":[]}', encoding="utf-8")
    (workflow_dir / "verify.json").symlink_to(outside)
    monkeypatch.setattr(
        "deepagents_code.sessions.get_workflows_dir", lambda: workflow_dir
    )

    with pytest.raises(ValueError, match="profile directory"):
        ProfileWorkflowRepository().load("verify")


@pytest.mark.asyncio
async def test_workflow_list_command_shows_saved_definitions(tmp_path, monkeypatch):
    from textual.screen import ModalScreen

    from deepagents_code.app import DeepAgentsApp
    from deepagents_code.application.workflows import ProfileWorkflowRepository

    monkeypatch.setattr(
        "deepagents_code.sessions.get_workflows_dir", lambda: tmp_path / "workflows"
    )
    ProfileWorkflowRepository().save(
        WorkflowDefinition("verify", 1, (WorkflowStep("inspect", "inspect"),))
    )

    class AppHarness:
        def __init__(self) -> None:
            self.messages: list[object] = []
            self.screen = ModalScreen()

        async def _mount_message(self, message: object) -> None:
            self.messages.append(message)

    harness = AppHarness()
    harness._handle_workflow_command = MethodType(
        DeepAgentsApp._handle_workflow_command, harness
    )
    await DeepAgentsApp._handle_command(harness, "/WORKFLOW LIST")

    assert len(harness.messages) == 1
    assert "verify" in str(harness.messages[0]._content)


def test_repository_load_save():
    repo = InMemoryWorkflowRepository()
    defn = WorkflowDefinition(name="w", version=1, steps=(WorkflowStep("a", "x"),))
    repo.save(defn)
    assert repo.load("w") == defn
    assert repo.load("missing") is None
    assert repo.list() == (defn,)
