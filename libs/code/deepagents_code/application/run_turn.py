"""Application-level coordinator for validated agent turns."""

from __future__ import annotations

from collections.abc import AsyncIterator

from deepagents_code.application.ports.runtime import AgentRuntime
from deepagents_code.domain.events import (
    ResumeRequest,
    RunEvent,
    RunEventKind,
    TurnRequest,
    TurnStatus,
)


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
    def __init__(self, runtime: AgentRuntime) -> None:
        self.runtime = runtime

    def stream_turn(self, request: TurnRequest) -> AsyncIterator[RunEvent]:
        return self._validate(
            request,
            self.runtime.stream_turn(request),
            initial_status=TurnStatus.created,
            last_sequence=0,
        )

    def resume_turn(self, request: ResumeRequest) -> AsyncIterator[RunEvent]:
        if request.prior_status not in (TurnStatus.running, TurnStatus.waiting_for_approval):
            raise RunProtocolError(
                f"resume requires running or waiting_for_approval, got {request.prior_status}"
            )
        return self._validate(
            request,
            self.runtime.resume_turn(request),
            initial_status=request.prior_status,
            last_sequence=request.last_sequence,
        )

    async def cancel_turn(self, session_id: str, turn_id: str) -> None:
        await self.runtime.cancel_turn(session_id, turn_id)

    async def _validate(
        self,
        request: TurnRequest | ResumeRequest,
        source: AsyncIterator[RunEvent],
        initial_status: TurnStatus,
        last_sequence: int,
    ) -> AsyncIterator[RunEvent]:
        status = initial_status
        last_seq = last_sequence
        terminal_seen = False
        last_kind: RunEventKind | None = None
        async for event in source:
            if event.session_id != request.session_id or event.turn_id != request.turn_id:
                raise RunProtocolError("cross-session or cross-turn event")
            if event.sequence <= last_seq:
                raise RunProtocolError("non-increasing sequence number")
            if terminal_seen:
                raise RunProtocolError("event after terminal event")
            new_status = _new_status(status, event.kind)
            if (status, new_status) not in _ALLOWED_TRANSITIONS:
                raise RunProtocolError(
                    f"illegal status transition {status} -> {new_status}"
                )
            status = new_status
            last_seq = event.sequence
            last_kind = event.kind
            if event.kind in _TERMINAL_KINDS:
                terminal_seen = True
            yield event
        if not (terminal_seen or last_kind == RunEventKind.approval_required):
            raise RunProtocolError(
                "stream closed without terminal event or approval pause"
            )
