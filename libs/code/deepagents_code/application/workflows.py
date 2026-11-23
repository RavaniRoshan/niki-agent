"""Workflow runner and repository."""

from __future__ import annotations

import asyncio
from collections.abc import AsyncIterator
from typing import Any

from deepagents_code.application.ports.workflows import WorkflowRepository
from deepagents_code.domain.events import RunEvent, RunEventKind
from deepagents_code.domain.workflow import WorkflowDefinition


class InMemoryWorkflowRepository:
    def __init__(self) -> None:
        self._store: dict[str, WorkflowDefinition] = {}

    def load(self, name: str) -> WorkflowDefinition | None:
        return self._store.get(name)

    def save(self, definition: WorkflowDefinition) -> None:
        self._store[definition.name] = definition


class WorkflowRunner:
    def __init__(self, repository: WorkflowRepository | None = None) -> None:
        self.repository = repository or InMemoryWorkflowRepository()

    async def run(
        self,
        definition: WorkflowDefinition,
        session_id: str,
        *,
        completed_steps: tuple[str, ...] = (),
    ) -> AsyncIterator[RunEvent]:
        completed = set(completed_steps)
        step_map = {s.step_id: s for s in definition.steps}
        # topological levels
        remaining = {s.step_id: set(s.depends_on) for s in definition.steps}
        for sid in completed:
            for other in remaining:
                remaining[other].discard(sid)

        seq = 0
        status: dict[str, str] = {sid: "completed" for sid in completed}

        async def run_step(step_id: str) -> tuple[str, str]:
            step = step_map[step_id]
            await asyncio.sleep(0)
            return step_id, f"result of {step_id}: {step.instruction}"

        while len(status) < len(definition.steps):
            # ready steps
            ready = [
                sid for sid in remaining
                if sid not in status and not (remaining[sid] - set(status))
            ]
            # remove already completed deps
            ready = [sid for sid in ready if sid not in status]
            if not ready:
                break
            # run ready in parallel
            tasks = [asyncio.ensure_future(run_step(sid)) for sid in ready]
            results = await asyncio.gather(*tasks)
            for step_id, res in results:
                status[step_id] = "completed"
                seq += 1
                yield RunEvent(
                    schema_version=1,
                    event_id=f"{step_id}-finished",
                    session_id=session_id,
                    turn_id=definition.name,
                    sequence=seq,
                    kind=RunEventKind.task_finished,
                    payload={"step_id": step_id, "result": res},
                )
            # update remaining deps
            for sid in list(remaining):
                remaining[sid] -= set(status)

        seq += 1
        yield RunEvent(
            schema_version=1,
            event_id=f"{definition.name}-workflow-finished",
            session_id=session_id,
            turn_id=definition.name,
            sequence=seq,
            kind=RunEventKind.workflow_finished,
            payload={"completed": list(status.keys())},
        )
