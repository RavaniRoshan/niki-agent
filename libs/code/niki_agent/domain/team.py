"""Framework-free agent team coordination types and invariants."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import TYPE_CHECKING, Literal, Protocol

if TYPE_CHECKING:
    from collections.abc import Sequence

    from niki_agent.domain.events import JsonObject

TeamTaskStatus = Literal["completed", "failed", "cancelled", "skipped"]


def _validate_json_object_keys(value: object) -> None:
    if isinstance(value, dict):
        for key, nested in value.items():
            if not isinstance(key, str):
                msg = "message body object keys must be strings"
                raise TeamProtocolError(msg)
            _validate_json_object_keys(nested)
    elif isinstance(value, list):
        for nested in value:
            _validate_json_object_keys(nested)


class TeamProtocolError(ValueError):
    """Raised when a team plan, checkpoint, or message violates its contract."""


@dataclass(frozen=True, slots=True)
class TeamTask:
    """One task assigned to a named agent within a team plan."""

    task_id: str
    agent_name: str
    prompt: str
    depends_on: tuple[str, ...] = ()


@dataclass(frozen=True, slots=True)
class TeamPlan:
    """A validated task DAG and its maximum active worker count."""

    team_id: str
    tasks: tuple[TeamTask, ...]
    max_parallel: int = 4

    def __post_init__(self) -> None:
        """Reject malformed task graphs at the domain boundary."""
        validate_team_plan(self)


@dataclass(frozen=True, slots=True)
class TeamTaskResult:
    """Terminal outcome for one team task."""

    task_id: str
    status: TeamTaskStatus
    summary: str

    def __post_init__(self) -> None:
        """Ensure persisted task results have valid terminal values.

        Raises:
            TeamProtocolError: If the task identity, status, or summary is invalid.
        """
        if not isinstance(self.task_id, str) or not self.task_id.strip():
            msg = "task result id must be a non-empty string"
            raise TeamProtocolError(msg)
        if not isinstance(self.status, str) or self.status not in {
            "completed",
            "failed",
            "cancelled",
            "skipped",
        }:
            msg = "task result status must be terminal"
            raise TeamProtocolError(msg)
        if not isinstance(self.summary, str):
            msg = "task result summary must be a string"
            raise TeamProtocolError(msg)


@dataclass(frozen=True, slots=True)
class TeamMessage:
    """A team message; sequence zero is an unsequenced message draft."""

    team_id: str
    message_id: str
    sequence: int
    sender_task_id: str
    recipient_task_id: str
    body: JsonObject

    def __post_init__(self) -> None:
        """Validate and detach message data from mutable caller objects.

        Raises:
            TeamProtocolError: If message identity, sequence, or body is invalid.
        """
        if not all(
            isinstance(value, str) and value.strip()
            for value in (
                self.team_id,
                self.message_id,
                self.sender_task_id,
                self.recipient_task_id,
            )
        ):
            msg = "message identity fields must be non-empty strings"
            raise TeamProtocolError(msg)
        if (
            isinstance(self.sequence, bool)
            or not isinstance(self.sequence, int)
            or self.sequence < 0
        ):
            msg = "message sequence must be an integer >= 0"
            raise TeamProtocolError(msg)
        if not isinstance(self.body, dict):
            msg = "message body must be a JSON object"
            raise TeamProtocolError(msg)
        _validate_json_object_keys(self.body)
        try:
            normalized = json.loads(json.dumps(self.body, allow_nan=False))
        except (TypeError, ValueError) as exc:
            msg = "message body must contain only JSON values"
            raise TeamProtocolError(msg) from exc
        object.__setattr__(self, "body", normalized)


@dataclass(frozen=True, slots=True)
class TeamCheckpoint:
    """Serializable team state kept in the owning session checkpoint."""

    team_id: str
    task_ids: tuple[str, ...]
    results: tuple[TeamTaskResult, ...] = ()
    messages: tuple[TeamMessage, ...] = ()
    next_message_sequence: int = 0
    event_sequence: int = 0

    def __post_init__(self) -> None:
        """Reject malformed checkpoint counters and task identities.

        Raises:
            TeamProtocolError: If checkpoint identities or counters are invalid.
        """
        if not isinstance(self.team_id, str) or not self.team_id.strip():
            msg = "checkpoint team id must be a non-empty string"
            raise TeamProtocolError(msg)
        if not isinstance(self.task_ids, tuple) or any(
            not isinstance(task_id, str) or not task_id.strip()
            for task_id in self.task_ids
        ):
            msg = "checkpoint task ids must be non-empty strings"
            raise TeamProtocolError(msg)
        if len(set(self.task_ids)) != len(self.task_ids):
            msg = "checkpoint task ids must be unique"
            raise TeamProtocolError(msg)
        if not isinstance(self.results, tuple) or any(
            not isinstance(result, TeamTaskResult) for result in self.results
        ):
            msg = "checkpoint results must be a tuple of TeamTaskResult values"
            raise TeamProtocolError(msg)
        if not isinstance(self.messages, tuple) or any(
            not isinstance(message, TeamMessage) for message in self.messages
        ):
            msg = "checkpoint messages must be a tuple of TeamMessage values"
            raise TeamProtocolError(msg)
        for counter in (self.next_message_sequence, self.event_sequence):
            if isinstance(counter, bool) or not isinstance(counter, int) or counter < 0:
                msg = "checkpoint sequence counters must be integers >= 0"
                raise TeamProtocolError(msg)


class TeamChannel(Protocol):
    """Checkpoint-backed message exchange scoped to a team member."""

    async def send(self, message: TeamMessage) -> None:
        """Persist a message and assign its server-owned sequence number."""

    async def receive(
        self, team_id: str, recipient_task_id: str, after_sequence: int
    ) -> Sequence[TeamMessage]:
        """Read this recipient's ordered messages after a checkpoint cursor."""


def validate_team_plan(plan: TeamPlan) -> None:
    """Validate stable task identities, dependencies, and the concurrency cap.

    Raises:
        TeamProtocolError: If a task, dependency, or concurrency limit is invalid.
    """
    if not isinstance(plan.team_id, str) or not plan.team_id.strip():
        msg = "team_id must be a non-empty string"
        raise TeamProtocolError(msg)
    if (
        isinstance(plan.max_parallel, bool)
        or not isinstance(plan.max_parallel, int)
        or plan.max_parallel < 1
    ):
        msg = "max_parallel must be an integer >= 1"
        raise TeamProtocolError(msg)

    if not isinstance(plan.tasks, tuple) or any(
        not isinstance(task, TeamTask) for task in plan.tasks
    ):
        msg = "tasks must be a tuple of TeamTask values"
        raise TeamProtocolError(msg)

    for task in plan.tasks:
        if not isinstance(task.agent_name, str) or not task.agent_name.strip():
            msg = f"agent name for task {task.task_id!r} must be non-empty"
            raise TeamProtocolError(msg)
        if not isinstance(task.prompt, str) or not task.prompt.strip():
            msg = f"prompt for task {task.task_id!r} must be non-empty"
            raise TeamProtocolError(msg)
        if not isinstance(task.depends_on, tuple) or any(
            not isinstance(dependency, str) or not dependency.strip()
            for dependency in task.depends_on
        ):
            msg = f"dependencies for task {task.task_id!r} must be task id strings"
            raise TeamProtocolError(msg)

    task_ids = [task.task_id for task in plan.tasks]
    if any(not isinstance(task_id, str) or not task_id.strip() for task_id in task_ids):
        msg = "task ids must be non-empty strings"
        raise TeamProtocolError(msg)
    if len(set(task_ids)) != len(task_ids):
        msg = "duplicate task ids"
        raise TeamProtocolError(msg)

    known_ids = set(task_ids)
    dependencies = {task.task_id: task.depends_on for task in plan.tasks}
    for task_id, task_dependencies in dependencies.items():
        if len(set(task_dependencies)) != len(task_dependencies):
            msg = f"duplicate dependencies for task {task_id}"
            raise TeamProtocolError(msg)
        for dependency in task_dependencies:
            if dependency not in known_ids:
                msg = f"unknown dependency {dependency}"
                raise TeamProtocolError(msg)

    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(task_id: str) -> None:
        if task_id in visiting:
            msg = "cycle detected"
            raise TeamProtocolError(msg)
        if task_id in visited:
            return
        visiting.add(task_id)
        for dependency in dependencies[task_id]:
            visit(dependency)
        visiting.remove(task_id)
        visited.add(task_id)

    for task_id in task_ids:
        visit(task_id)
