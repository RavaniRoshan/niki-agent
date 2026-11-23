"""Adapter from the direct ACP graph path to Niki runtime events."""

from __future__ import annotations

from collections.abc import AsyncIterator
from typing import Any

from deepagents_code.application.ports.runtime import AgentRuntime
from deepagents_code.application.run_turn import RunProtocolError
from deepagents_code.domain.events import (
    ResumeRequest,
    RunEvent,
    RunEventKind,
    TurnRequest,
)


class ACPAgentRuntime:
    """AgentRuntime for the existing direct ACP graph path."""

    def __init__(self, agent: Any, session: Any) -> None:
        self.agent = agent
        self.session = session

    async def stream_turn(self, request: TurnRequest) -> AsyncIterator[RunEvent]:
        yield RunEvent(
            schema_version=1,
            event_id=f"{request.turn_id}-started",
            session_id=request.session_id,
            turn_id=request.turn_id,
            sequence=1,
            kind=RunEventKind.turn_started,
            payload={},
        )
        try:
            async for _ in self.agent.astream(
                {"messages": [{"role": "user", "content": request.prompt}]},
                config={"configurable": {"thread_id": request.session_id}},
            ):
                pass
        except Exception as exc:
            yield RunEvent(
                schema_version=1,
                event_id=f"{request.turn_id}-failed",
                session_id=request.session_id,
                turn_id=request.turn_id,
                sequence=2,
                kind=RunEventKind.turn_failed,
                payload={"error": str(exc)},
            )
            return
        yield RunEvent(
            schema_version=1,
            event_id=f"{request.turn_id}-finished",
            session_id=request.session_id,
            turn_id=request.turn_id,
            sequence=2,
            kind=RunEventKind.turn_finished,
            payload={},
        )

    async def resume_turn(self, request: ResumeRequest) -> AsyncIterator[RunEvent]:
        seq = request.last_sequence
        yield RunEvent(
            schema_version=1,
            event_id=f"{request.turn_id}-resumed",
            session_id=request.session_id,
            turn_id=request.turn_id,
            sequence=seq + 1,
            kind=RunEventKind.session_resumed,
            payload={},
        )
        yield RunEvent(
            schema_version=1,
            event_id=f"{request.turn_id}-finished",
            session_id=request.session_id,
            turn_id=request.turn_id,
            sequence=seq + 2,
            kind=RunEventKind.turn_finished,
            payload={},
        )

    async def cancel_turn(self, session_id: str, turn_id: str) -> None:
        cancel = getattr(self.agent, "acancel_active_runs", None)
        if cancel is not None:
            await cancel({"configurable": {"thread_id": session_id}})
