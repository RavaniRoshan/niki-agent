"""Framework-free reusable workflow definitions and run references."""

from __future__ import annotations

import re
from dataclasses import dataclass

_WORKFLOW_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$")
_WORKFLOW_RESULT_PARTS = 2


@dataclass(frozen=True, slots=True)
class WorkflowStep:
    """One prompt staged behind zero or more prerequisite steps."""

    step_id: str
    instruction: str
    depends_on: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        """Reject empty ids, instructions, or malformed dependency lists.

        Raises:
            ValueError: If a step field is empty or malformed.
        """
        if not isinstance(self.step_id, str) or not self.step_id.strip():
            msg = "workflow step id must be a non-empty string"
            raise ValueError(msg)
        if not isinstance(self.instruction, str) or not self.instruction.strip():
            msg = "workflow step instruction must be non-empty"
            raise ValueError(msg)
        if not isinstance(self.depends_on, tuple) or any(
            not isinstance(value, str) or not value.strip() for value in self.depends_on
        ):
            msg = "workflow dependencies must be non-empty step ids"
            raise ValueError(msg)
        if len(set(self.depends_on)) != len(self.depends_on):
            msg = "workflow dependencies must be unique"
            raise ValueError(msg)


@dataclass(frozen=True, slots=True)
class WorkflowDefinition:
    """Validated, versioned DAG of reusable natural-language steps."""

    name: str
    version: int
    steps: tuple[WorkflowStep, ...]

    def __post_init__(self) -> None:
        """Validate workflow identity and ensure the step graph is acyclic.

        Raises:
            TypeError: If a step value has the wrong type.
            ValueError: If the workflow identity or graph is invalid.
        """
        if not isinstance(self.name, str) or not _WORKFLOW_NAME.fullmatch(self.name):
            msg = "workflow name must be 1-64 safe filename characters"
            raise ValueError(msg)
        if (
            isinstance(self.version, bool)
            or not isinstance(self.version, int)
            or self.version < 1
        ):
            msg = "workflow version must be an integer >= 1"
            raise ValueError(msg)
        if not isinstance(self.steps, tuple) or not self.steps:
            msg = "workflow must contain at least one step"
            raise ValueError(msg)
        if any(not isinstance(step, WorkflowStep) for step in self.steps):
            msg = "workflow steps must be WorkflowStep values"
            raise TypeError(msg)

        step_ids = [step.step_id for step in self.steps]
        if len(set(step_ids)) != len(step_ids):
            msg = "duplicate workflow step ids"
            raise ValueError(msg)
        known_ids = set(step_ids)
        dependencies = {step.step_id: step.depends_on for step in self.steps}
        for step in self.steps:
            for dependency in step.depends_on:
                if dependency not in known_ids:
                    msg = f"unknown workflow dependency {dependency!r}"
                    raise ValueError(msg)

        visiting: set[str] = set()
        visited: set[str] = set()

        def visit(step_id: str) -> None:
            if step_id in visiting:
                msg = "workflow cycle detected"
                raise ValueError(msg)
            if step_id in visited:
                return
            visiting.add(step_id)
            for dependency in dependencies[step_id]:
                visit(dependency)
            visiting.remove(step_id)
            visited.add(step_id)

        for step_id in step_ids:
            visit(step_id)


@dataclass(frozen=True, slots=True)
class WorkflowRun:
    """Workflow progress reference restored from the owning session checkpoint."""

    workflow_name: str
    session_id: str
    completed_steps: tuple[str, ...] = ()
    step_results: tuple[tuple[str, str], ...] = ()
    event_sequence: int = 0

    def __post_init__(self) -> None:
        """Validate the identity and unique completed-step references.

        Raises:
            ValueError: If the run identity or progress is invalid.
        """
        if not isinstance(self.workflow_name, str) or not _WORKFLOW_NAME.fullmatch(
            self.workflow_name
        ):
            msg = "workflow run name is invalid"
            raise ValueError(msg)
        if not isinstance(self.session_id, str) or not self.session_id.strip():
            msg = "workflow run session id must be non-empty"
            raise ValueError(msg)
        if not isinstance(self.completed_steps, tuple) or any(
            not isinstance(step_id, str) or not step_id.strip()
            for step_id in self.completed_steps
        ):
            msg = "completed workflow steps must be non-empty ids"
            raise ValueError(msg)
        if len(set(self.completed_steps)) != len(self.completed_steps):
            msg = "completed workflow steps must be unique"
            raise ValueError(msg)
        if not isinstance(self.step_results, tuple) or any(
            not isinstance(result, tuple)
            or len(result) != _WORKFLOW_RESULT_PARTS
            or not isinstance(result[0], str)
            or not isinstance(result[1], str)
            for result in self.step_results
        ):
            msg = "workflow step results must be (step id, text) pairs"
            raise ValueError(msg)
        result_ids = [step_id for step_id, _ in self.step_results]
        if len(set(result_ids)) != len(result_ids):
            msg = "workflow step results must have unique ids"
            raise ValueError(msg)
        if set(result_ids) != set(self.completed_steps):
            msg = "workflow step results must match completed steps"
            raise ValueError(msg)
        if (
            isinstance(self.event_sequence, bool)
            or not isinstance(self.event_sequence, int)
            or self.event_sequence < 0
        ):
            msg = "workflow event sequence must be an integer >= 0"
            raise ValueError(msg)
