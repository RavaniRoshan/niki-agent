"""Agent task execution port."""

from __future__ import annotations

from typing import Protocol

from deepagents_code.domain.team import TeamChannel, TeamTask, TeamTaskResult


class AgentTaskPort(Protocol):
    async def run(
        self, task: TeamTask, *, session_id: str, channel: TeamChannel
    ) -> TeamTaskResult:
        ...
