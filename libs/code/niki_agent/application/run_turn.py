"""Application-level coordinator for validated agent turns."""

from __future__ import annotations

from typing import TYPE_CHECKING

from niki_agent.domain.events import (
    ResumeRequest,
    RunEvent,
    RunEventKind,
    TurnRequest,
    TurnStatus,
)

if TYPE_CHECKING:
    from collections.abc import AsyncIterator

    from niki_agent.application.ports.runtime import AgentRuntime


class RunProtocolError(ValueError):
    """Raised when a streamed run violates the runtime protocol."""


_TERMINAL_KINDS = {
    RunEventKind.turn_finished,
    RunEventKind.turn_failed,
    RunEventKind.turn_cancelled,
}

_ALLOWED_TRANSITIONS: set[tuple[TurnStatus, TurnStatus]] = {
    (TurnStatus.created, TurnStatus.running),
    (TurnStatus.running, TurnStatus.running),
    (TurnStatus.running, TurnStatus.waiting_for_approval),
    (TurnStatus.running, TurnStatus.completed),
    (TurnStatus.running, TurnStatus.failed),
    (TurnStatus.running, TurnStatus.cancelled),
    (TurnStatus.waiting_for_approval, TurnStatus.running),
    (TurnStatus.waiting_for_approval, TurnStatus.waiting_for_approval),
    (TurnStatus.waiting_for_approval, TurnStatus.completed),
    (TurnStatus.waiting_for_approval, TurnStatus.failed),
    (TurnStatus.waiting_for_approval, TurnStatus.cancelled),
}


def _new_status(current: TurnStatus, kind: RunEventKind) -> TurnStatus:
    if kind == RunEventKind.turn_started:
        return TurnStatus.running
    if kind == RunEventKind.approval_required:
        return TurnStatus.waiting_for_approval
    if kind == RunEventKind.turn_finished:
        return TurnStatus.completed
    if kind == RunEventKind.turn_failed:
        return TurnStatus.failed
    if kind == RunEventKind.turn_cancelled:
        return TurnStatus.cancelled
    return current


class RunTurnCoordinator:
    """Validate runtime event streams against the turn protocol."""

    def __init__(self, runtime: AgentRuntime) -> None:
        """Bind the coordinator to one runtime."""
        self.runtime = runtime

    def stream_turn(self, request: TurnRequest) -> AsyncIterator[RunEvent]:
        """Validate a new turn stream.

        Returns:
            The validated event iterator.
        """
        return self._validate(
            request,
            self.runtime.stream_turn(request),
            initial_status=TurnStatus.created,
            last_sequence=0,
        )

    def resume_turn(self, request: ResumeRequest) -> AsyncIterator[RunEvent]:
        """Validate a resumed turn stream.

        Returns:
            The validated event iterator.

        Raises:
            RunProtocolError: If the prior status cannot be resumed.
        """
        allowed = {TurnStatus.running, TurnStatus.waiting_for_approval}
        if request.prior_status not in allowed:
            msg = (
                "resume requires running or waiting_for_approval, "
                f"got {request.prior_status}"
            )
            raise RunProtocolError(msg)
        return self._validate(
            request,
            self.runtime.resume_turn(request),
            initial_status=request.prior_status,
            last_sequence=request.last_sequence,
        )

    async def cancel_turn(self, session_id: str, turn_id: str) -> None:
        """Cancel a turn through the runtime."""
        await self.runtime.cancel_turn(session_id, turn_id)

    @staticmethod
    async def _validate(
        request: TurnRequest | ResumeRequest,
        source: AsyncIterator[RunEvent],
        initial_status: TurnStatus,
        last_sequence: int,
    ) -> AsyncIterator[RunEvent]:
        """Yield events only while transitions and ordering stay legal.

        Raises:
            RunProtocolError: If the stream violates the turn protocol.
        """
        status = initial_status
        last_seq = last_sequence
        terminal_seen = False
        last_kind: RunEventKind | None = None
        async for event in source:
            same_turn = (
                event.session_id == request.session_id
                and event.turn_id == request.turn_id
            )
            if not same_turn:
                msg = "cross-session or cross-turn event"
                raise RunProtocolError(msg)
            if event.sequence <= last_seq:
                msg = "non-increasing sequence number"
                raise RunProtocolError(msg)
            if terminal_seen:
                msg = "event after terminal event"
                raise RunProtocolError(msg)
            new_status = _new_status(status, event.kind)
            if (status, new_status) not in _ALLOWED_TRANSITIONS:
                msg = f"illegal status transition {status} -> {new_status}"
                raise RunProtocolError(msg)
            status = new_status
            last_seq = event.sequence
            last_kind = event.kind
            if event.kind in _TERMINAL_KINDS:
                terminal_seen = True
            yield event
        if not (terminal_seen or last_kind == RunEventKind.approval_required):
            msg = "stream closed without terminal event or approval pause"
            raise RunProtocolError(msg)
