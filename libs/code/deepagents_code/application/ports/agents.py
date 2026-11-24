"""Agent task execution port."""

from __future__ import annotations

from typing import TYPE_CHECKING, Protocol

if TYPE_CHECKING:
    from deepagents_code.domain.team import (
        TeamChannel,
        TeamCheckpoint,
        TeamTask,
        TeamTaskResult,
    )


class AgentTaskPort(Protocol):
    """Execute one independent team task in its own task context."""

    async def run(
        self, task: TeamTask, *, session_id: str, channel: TeamChannel
    ) -> TeamTaskResult:
        """Run a task using its own context and scoped team channel."""


class TeamCheckpointPort(Protocol):
    """Read and write team state through the owning session checkpointer."""

    async def load(self, session_id: str, team_id: str) -> TeamCheckpoint | None:
        """Return the latest checkpoint for one session-scoped team."""

    async def save(self, session_id: str, checkpoint: TeamCheckpoint) -> None:
        """Atomically replace the team state in its owning session checkpoint."""
