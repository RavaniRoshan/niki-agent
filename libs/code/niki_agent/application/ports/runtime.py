"""Agent runtime port."""

from __future__ import annotations

from typing import TYPE_CHECKING, Protocol, runtime_checkable

if TYPE_CHECKING:
    from collections.abc import AsyncIterator

    from niki_agent.domain.events import (
        ResumeRequest,
        RunEvent,
        TurnRequest,
    )


@runtime_checkable
class AgentRuntime(Protocol):
    """Runtime boundary that streams validated turn events."""

    def stream_turn(self, request: TurnRequest) -> AsyncIterator[RunEvent]:
        """Stream events for a new turn."""
        ...

    def resume_turn(self, request: ResumeRequest) -> AsyncIterator[RunEvent]:
        """Stream events after an approval decision."""
        ...

    async def cancel_turn(self, session_id: str, turn_id: str) -> None:
        """Cancel a running turn."""
        ...
