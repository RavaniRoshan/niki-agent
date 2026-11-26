"""Workflow repository adapters and staged execution use case."""

from __future__ import annotations

import asyncio
import json
import os
import re
import tempfile
from pathlib import Path
from typing import TYPE_CHECKING
from uuid import uuid4

from niki_agent.domain.events import RunEvent, RunEventKind
from niki_agent.domain.workflow import (
    WorkflowDefinition,
    WorkflowRun,
    WorkflowStep,
)

if TYPE_CHECKING:
    from collections.abc import AsyncIterator

    from niki_agent.application.ports.workflows import (
        WorkflowStepPort,
    )
    from niki_agent.domain.events import JsonObject

_SAFE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$")


class InMemoryWorkflowRepository:
    """In-memory workflow repository for isolated use-case tests."""

    def __init__(self) -> None:
        """Create an empty repository."""
        self._store: dict[str, WorkflowDefinition] = {}

    def load(self, name: str) -> WorkflowDefinition | None:
        """Return a saved definition, if present."""
        return self._store.get(name)

    def save(self, definition: WorkflowDefinition) -> None:
        """Save a definition by name."""
        self._store[definition.name] = definition

    def list(self) -> tuple[WorkflowDefinition, ...]:
        """Return saved definitions in stable name order."""
        return tuple(self._store[name] for name in sorted(self._store))


class ProfileWorkflowRepository:
    """Atomically persist JSON workflow definitions in the active profile."""

    @staticmethod
    def _path(name: str) -> tuple[Path, Path]:
        if not isinstance(name, str) or not _SAFE_NAME.fullmatch(name):
            msg = "workflow name must be 1-64 safe filename characters"
            raise ValueError(msg)
        from niki_agent.sessions import get_workflows_dir

        directory = get_workflows_dir()
        directory.mkdir(parents=True, exist_ok=True, mode=0o700)
        root = directory.resolve()
        path = directory / f"{name}.json"
        if path.parent.resolve() != root or path.is_symlink():
            msg = "workflow path must remain inside the profile directory"
            raise ValueError(msg)
        return directory, path

    def load(self, name: str) -> WorkflowDefinition | None:
        """Load and validate a saved JSON definition.

        Returns:
            The saved definition, or `None` if the name is not saved.

        Raises:
            TypeError: If a decoded definition field has the wrong type.
            ValueError: If the name or saved JSON document is invalid.
        """
        _, path = self._path(name)
        try:
            raw = path.read_text(encoding="utf-8")
        except FileNotFoundError:
            return None
        try:
            document = json.loads(raw)
        except json.JSONDecodeError as exc:
            msg = f"workflow {name!r} has invalid JSON"
            raise ValueError(msg) from exc
        if not isinstance(document, dict):
            msg = "workflow document must be an object"
            raise TypeError(msg)
        if set(document) != {"name", "version", "steps"}:
            msg = "workflow document has invalid fields"
            raise ValueError(msg)
        raw_steps = document["steps"]
        if not isinstance(raw_steps, list):
            msg = "workflow steps must be a list"
            raise TypeError(msg)
        steps = []
        for item in raw_steps:
            if not isinstance(item, dict):
                msg = "workflow step must be an object"
                raise TypeError(msg)
            if set(item) != {"step_id", "instruction", "depends_on"}:
                msg = "workflow step has invalid fields"
                raise ValueError(msg)
            dependencies = item["depends_on"]
            if not isinstance(dependencies, list):
                msg = "workflow dependencies must be a list"
                raise TypeError(msg)
            steps.append(
                WorkflowStep(
                    step_id=item["step_id"],
                    instruction=item["instruction"],
                    depends_on=tuple(dependencies),
                )
            )
        definition = WorkflowDefinition(
            name=document["name"],
            version=document["version"],
            steps=tuple(steps),
        )
        if definition.name != name:
            msg = "workflow file name does not match its definition"
            raise ValueError(msg)
        return definition

    def save(self, definition: WorkflowDefinition) -> None:
        """Write a validated definition with an atomic file replacement."""
        directory, path = self._path(definition.name)
        payload = {
            "name": definition.name,
            "version": definition.version,
            "steps": [
                {
                    "step_id": step.step_id,
                    "instruction": step.instruction,
                    "depends_on": list(step.depends_on),
                }
                for step in definition.steps
            ],
        }
        temporary_path = None
        try:
            with tempfile.NamedTemporaryFile(
                mode="w",
                encoding="utf-8",
                dir=directory,
                prefix=f".{definition.name}.",
                suffix=".tmp",
                delete=False,
            ) as file:
                temporary_path = Path(file.name)
                json.dump(payload, file, ensure_ascii=False, indent=2)
                file.write("\n")
                file.flush()
                os.fsync(file.fileno())
            temporary_path.chmod(0o600)
            temporary_path.replace(path)
        finally:
            if temporary_path is not None and temporary_path.exists():
                temporary_path.unlink()

    def list(self) -> tuple[WorkflowDefinition, ...]:
        """Return all valid saved definitions in stable name order.

        Raises:
            ValueError: If a saved definition is malformed or is a symlink.
        """
        from niki_agent.sessions import get_workflows_dir

        directory = get_workflows_dir()
        if not directory.exists():
            return ()
        definitions = []
        for path in sorted(directory.glob("*.json")):
            if path.is_symlink():
                msg = "workflow files cannot be symbolic links"
                raise ValueError(msg)
            definition = self.load(path.stem)
            if definition is not None:
                definitions.append(definition)
        return tuple(definitions)


class WorkflowRunner:
    """Execute ready workflow stages through an injected agent capability.

    The caller restores `WorkflowRun` from and persists progress into the
    owning LangGraph session. This runner does not create a second state store.
    """

    def __init__(self, *, step_port: WorkflowStepPort) -> None:
        """Bind the workflow runner to the existing agent execution capability."""
        self.step_port = step_port

    async def run(
        self,
        definition: WorkflowDefinition,
        session_id: str,
        *,
        run: WorkflowRun | None = None,
    ) -> AsyncIterator[RunEvent]:
        """Run ready steps concurrently and emit ordered lifecycle events.

        A failed stage stops its dependents; completed peer steps remain visible
        in the failure event so the caller can checkpoint them.

        Yields:
            Workflow and task lifecycle events with monotonically increasing
            sequence numbers for this invocation.

        Raises:
            ValueError: If the session or restored progress does not match.
            RuntimeError: If a valid workflow has no runnable step.
        """
        if not isinstance(session_id, str) or not session_id.strip():
            msg = "session id must be non-empty"
            raise ValueError(msg)
        progress = run or WorkflowRun(definition.name, session_id)
        if (
            progress.workflow_name != definition.name
            or progress.session_id != session_id
        ):
            msg = "workflow progress belongs to a different run"
            raise ValueError(msg)
        step_map = {step.step_id: step for step in definition.steps}
        if not set(progress.completed_steps) <= step_map.keys():
            msg = "workflow progress contains an unknown step"
            raise ValueError(msg)

        completed = set(progress.completed_steps)
        results = dict(progress.step_results)
        sequence = progress.event_sequence
        active: dict[asyncio.Task[str], WorkflowStep] = {}
        step_order = {
            step.step_id: index for index, step in enumerate(definition.steps)
        }

        def event(kind: RunEventKind, payload: JsonObject) -> RunEvent:
            nonlocal sequence
            sequence += 1
            return RunEvent(
                schema_version=1,
                event_id=uuid4().hex,
                session_id=session_id,
                turn_id=definition.name,
                sequence=sequence,
                kind=kind,
                payload=payload,
            )

        try:
            yield event(
                RunEventKind.workflow_started,
                {
                    "workflow_name": definition.name,
                    "version": definition.version,
                    "event_sequence": sequence + 1,
                    "completed_steps": [
                        step.step_id
                        for step in definition.steps
                        if step.step_id in completed
                    ],
                },
            )

            while len(completed) < len(step_map):
                ready = [
                    step
                    for step in definition.steps
                    if step.step_id not in completed
                    and set(step.depends_on) <= completed
                ]
                if not ready:
                    msg = "workflow has no runnable step"
                    raise RuntimeError(msg)

                for step in ready:
                    task = asyncio.create_task(
                        self.step_port.run_step(
                            step,
                            session_id=session_id,
                            workflow_name=definition.name,
                        )
                    )
                    active[task] = step
                    yield event(
                        RunEventKind.task_started,
                        {"step_id": step.step_id, "workflow_name": definition.name},
                    )

                done, pending = await asyncio.wait(
                    active, return_when=asyncio.FIRST_EXCEPTION
                )
                failures = [
                    task
                    for task in done
                    if task.cancelled()
                    or task.exception() is not None
                    or not isinstance(task.result(), str)
                ]
                if failures:
                    pending_tasks = sorted(
                        pending,
                        key=lambda item: step_order[active[item].step_id],
                    )
                    for task in pending_tasks:
                        task.cancel()
                    pending_results = await asyncio.gather(
                        *pending_tasks, return_exceptions=True
                    )
                    failed_steps = {active[task].step_id for task in failures}
                    for task in sorted(
                        done, key=lambda item: step_order[active[item].step_id]
                    ):
                        step = active.pop(task)
                        if task.cancelled():
                            yield event(
                                RunEventKind.task_failed,
                                {"step_id": step.step_id, "error": "step cancelled"},
                            )
                            continue
                        error = task.exception()
                        if error is None:
                            result = task.result()
                            if not isinstance(result, str):
                                msg = "workflow step result must be text"
                                error = TypeError(msg)
                            else:
                                completed.add(step.step_id)
                                results[step.step_id] = result
                                yield event(
                                    RunEventKind.task_finished,
                                    {"step_id": step.step_id, "result": result},
                                )
                                continue
                        yield event(
                            RunEventKind.task_failed,
                            {"step_id": step.step_id, "error": str(error)},
                        )
                    for task, result in zip(
                        pending_tasks, pending_results, strict=True
                    ):
                        step = active.pop(task)
                        if isinstance(result, str):
                            completed.add(step.step_id)
                            results[step.step_id] = result
                            yield event(
                                RunEventKind.task_finished,
                                {"step_id": step.step_id, "result": result},
                            )
                        else:
                            error = (
                                "cancelled after peer failure"
                                if isinstance(result, asyncio.CancelledError)
                                else str(result)
                            )
                            yield event(
                                RunEventKind.task_failed,
                                {"step_id": step.step_id, "error": error},
                            )
                    failed_step = next(
                        step.step_id for step in ready if step.step_id in failed_steps
                    )
                    yield event(
                        RunEventKind.workflow_failed,
                        {
                            "workflow_name": definition.name,
                            "failed_step": failed_step,
                            "event_sequence": sequence + 1,
                            "completed_steps": [
                                step.step_id
                                for step in definition.steps
                                if step.step_id in completed
                            ],
                            "results": {
                                step.step_id: results[step.step_id]
                                for step in definition.steps
                                if step.step_id in results
                            },
                        },
                    )
                    return

                for task in sorted(
                    done, key=lambda item: step_order[active[item].step_id]
                ):
                    step = active.pop(task)
                    result = task.result()
                    completed.add(step.step_id)
                    results[step.step_id] = result
                    yield event(
                        RunEventKind.task_finished,
                        {"step_id": step.step_id, "result": result},
                    )

            yield event(
                RunEventKind.workflow_finished,
                {
                    "workflow_name": definition.name,
                    "version": definition.version,
                    "event_sequence": sequence + 1,
                    "completed_steps": [
                        step.step_id
                        for step in definition.steps
                        if step.step_id in completed
                    ],
                    "results": {
                        step.step_id: results[step.step_id]
                        for step in definition.steps
                        if step.step_id in results
                    },
                },
            )
        finally:
            for task in active:
                task.cancel()
            if active:
                await asyncio.gather(*active, return_exceptions=True)
