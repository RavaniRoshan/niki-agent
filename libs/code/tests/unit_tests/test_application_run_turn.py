"""Tests for the application run coordinator."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

import pytest

from deepagents_code.application.run_turn import RunProtocolError, RunTurnCoordinator
from deepagents_code.domain.events import (
    ResumeRequest,
    RunEvent,
    RunEventKind,
    TurnRequest,
    TurnStatus,
)

if TYPE_CHECKING:
    from collections.abc import AsyncIterator


class FakeRuntime:
    def __init__(self, events: list[RunEvent]) -> None:
        self._events = events
        self.cancelled: list[tuple[str, str]] = []

    async def stream_turn(self, _request: TurnRequest) -> AsyncIterator[RunEvent]:
        for e in self._events:
            yield e

    async def resume_turn(self, _request: ResumeRequest) -> AsyncIterator[RunEvent]:
        for e in self._events:
            yield e

    async def cancel_turn(self, session_id: str, turn_id: str) -> None:
        self.cancelled.append((session_id, turn_id))


def _ev(seq: int, kind: RunEventKind, sid: str = "s1", tid: str = "t1") -> RunEvent:
    return RunEvent(
        schema_version=1,
        event_id=f"e{seq}",
        session_id=sid,
        turn_id=tid,
        sequence=seq,
        kind=kind,
        payload={},
    )


@pytest.mark.asyncio
async def test_run_turn_streams_events_until_terminal_event():
    events = [
        _ev(1, RunEventKind.turn_started),
        _ev(2, RunEventKind.text_delta),
        _ev(3, RunEventKind.turn_finished),
    ]
    coord = RunTurnCoordinator(FakeRuntime(events))
    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")
    out = [e async for e in coord.stream_turn(req)]
    assert [e.kind for e in out] == [
        RunEventKind.turn_started,
        RunEventKind.text_delta,
        RunEventKind.turn_finished,
    ]


@pytest.mark.asyncio
async def test_run_turn_accepts_approval_pause_and_resume():
    events1 = [
        _ev(1, RunEventKind.turn_started),
        _ev(2, RunEventKind.approval_required),
    ]
    coord = RunTurnCoordinator(FakeRuntime(events1))
    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")
    out1 = [e async for e in coord.stream_turn(req)]
    assert out1[-1].kind == RunEventKind.approval_required

    events2 = [
        _ev(3, RunEventKind.turn_started),
        _ev(4, RunEventKind.turn_finished),
    ]
    coord2 = RunTurnCoordinator(FakeRuntime(events2))
    rreq = ResumeRequest(
        session_id="s1",
        turn_id="t1",
        decision={},
        last_sequence=2,
        prior_status=TurnStatus.waiting_for_approval,
    )
    out2 = [e async for e in coord2.resume_turn(rreq)]
    assert out2[0].sequence == 3
    assert out2[-1].kind == RunEventKind.turn_finished


@pytest.mark.asyncio
async def test_run_turn_accepts_interrupted_run_resume():
    events = [
        _ev(3, RunEventKind.text_delta),
        _ev(4, RunEventKind.turn_finished),
    ]
    coord = RunTurnCoordinator(FakeRuntime(events))
    rreq = ResumeRequest(
        session_id="s1",
        turn_id="t1",
        decision={},
        last_sequence=2,
        prior_status=TurnStatus.running,
    )
    out = [e async for e in coord.resume_turn(rreq)]
    assert out[0].sequence == 3


@pytest.mark.asyncio
async def test_run_turn_rejects_cross_session_event():
    events = [_ev(1, RunEventKind.turn_started, sid="s2")]
    coord = RunTurnCoordinator(FakeRuntime(events))
    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")
    with pytest.raises(RunProtocolError):
        async for _ in coord.stream_turn(req):
            pass


@pytest.mark.asyncio
async def test_run_turn_rejects_non_increasing_sequence():
    events = [
        _ev(1, RunEventKind.turn_started),
        _ev(1, RunEventKind.text_delta),
    ]
    coord = RunTurnCoordinator(FakeRuntime(events))
    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")
    with pytest.raises(RunProtocolError):
        async for _ in coord.stream_turn(req):
            pass


@pytest.mark.asyncio
async def test_run_turn_rejects_illegal_status_transition():
    # turn_finished before turn_started
    events = [_ev(1, RunEventKind.turn_finished)]
    coord = RunTurnCoordinator(FakeRuntime(events))
    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")
    with pytest.raises(RunProtocolError):
        async for _ in coord.stream_turn(req):
            pass


@pytest.mark.asyncio
async def test_run_turn_rejects_stream_without_terminal_event():
    events = [
        _ev(1, RunEventKind.turn_started),
        _ev(2, RunEventKind.text_delta),
    ]
    coord = RunTurnCoordinator(FakeRuntime(events))
    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")
    with pytest.raises(RunProtocolError):
        async for _ in coord.stream_turn(req):
            pass


@pytest.mark.asyncio
async def test_cancel_turn_delegates():
    coord = RunTurnCoordinator(FakeRuntime([]))
    await coord.cancel_turn("s1", "t1")
    assert coord.runtime.cancelled == [("s1", "t1")]  # type: ignore[attr-defined]
