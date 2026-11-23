"""Agent runtime port."""

from __future__ import annotations

from collections.abc import AsyncIterator
from typing import Protocol

from deepagents_code.domain.events import (
    ResumeRequest,
    RunEvent,
    TurnRequest,
)


class AgentRuntime(Protocol):
    def stream_turn(self, request: TurnRequest) -> AsyncIterator[RunEvent]:
        ...

    def resume_turn(self, request: ResumeRequest) -> AsyncIterator[RunEvent]:
        ...

    async def cancel_turn(self, session_id: str, turn_id: str) -> None:
        ...
