"""Framework-free agent team coordination types."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal, Protocol

from deepagents_code.domain.events import JsonObject


TeamTaskStatus = Literal["completed", "failed", "cancelled", "skipped"]


@dataclass(frozen=True, slots=True)
class TeamTask:
    task_id: str
    agent_name: str
    prompt: str
    depends_on: tuple[str, ...] = ()


@dataclass(frozen=True, slots=True)
class TeamPlan:
    team_id: str
    tasks: tuple[TeamTask, ...]
    max_parallel: int = 4

    def __post_init__(self):
        from deepagents_code.application.orchestration.team import _validate_plan
        _validate_plan(self)


@dataclass(frozen=True, slots=True)
class TeamTaskResult:
    task_id: str
    status: TeamTaskStatus
    summary: str


@dataclass(frozen=True, slots=True)
class TeamMessage:
    team_id: str
    message_id: str
    sequence: int
    sender_task_id: str
    recipient_task_id: str
    body: JsonObject


class TeamChannel(Protocol):
    async def send(self, message: TeamMessage) -> None:
        ...

    async def receive(
        self, team_id: str, recipient_task_id: str, after_sequence: int
    ) -> list[TeamMessage]:
        ...
