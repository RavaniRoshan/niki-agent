"""Tests for forwarding native graph chunks alongside validated run events."""

from __future__ import annotations

import asyncio
from typing import TYPE_CHECKING

import pytest

from deepagents_code.application.native_stream import stream_native_chunks
from deepagents_code.application.run_turn import RunTurnCoordinator
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
    def __init__(self, queue: asyncio.Queue[object], events: list[RunEvent]) -> None:
        self.queue = queue
        self.events = events
        self.cancelled: list[tuple[str, str]] = []
        self.started = asyncio.Event()

    async def stream_turn(self, request: TurnRequest) -> AsyncIterator[RunEvent]:
        del request
        await self.queue.put("native-1")
        await self.queue.put("native-2")
        self.started.set()
        for event in self.events:
            yield event

    async def resume_turn(self, request: ResumeRequest) -> AsyncIterator[RunEvent]:
        del request
        for event in self.events:
            yield event

    async def cancel_turn(self, session_id: str, turn_id: str) -> None:
        self.cancelled.append((session_id, turn_id))


def _event(sequence: int, kind: RunEventKind) -> RunEvent:
    return RunEvent(
        schema_version=1,
        event_id=f"turn-1:{sequence}",
        session_id="session-1",
        turn_id="turn-1",
        sequence=sequence,
        kind=kind,
        payload={},
    )


@pytest.mark.asyncio
async def test_stream_native_chunks_preserves_observations_and_runtime_events():
    queue: asyncio.Queue[object] = asyncio.Queue(maxsize=1)
    runtime = FakeRuntime(
        queue,
        [
            _event(1, RunEventKind.turn_started),
            _event(2, RunEventKind.text_delta),
            _event(3, RunEventKind.turn_finished),
        ],
    )
    request = TurnRequest("session-1", "turn-1", "hello", "/workspace")
    received: list[RunEvent] = []

    chunks = [
        chunk
        async for chunk in stream_native_chunks(
            RunTurnCoordinator(runtime), request, queue, on_event=received.append
        )
    ]

    assert chunks == ["native-1", "native-2"]
    assert [event.kind for event in received] == [
        RunEventKind.turn_started,
        RunEventKind.text_delta,
        RunEventKind.turn_finished,
    ]
    assert runtime.cancelled == []


@pytest.mark.asyncio
async def test_stream_native_chunks_forwards_recoverable_runtime_failure():
    queue: asyncio.Queue[object] = asyncio.Queue(maxsize=1)
    runtime = FakeRuntime(
        queue,
        [
            _event(1, RunEventKind.turn_started),
            RunEvent(
                schema_version=1,
                event_id="turn-1:2",
                session_id="session-1",
                turn_id="turn-1",
                sequence=2,
                kind=RunEventKind.turn_failed,
                payload={"error": "provider timeout", "recoverable": True},
            ),
        ],
    )
    request = TurnRequest("session-1", "turn-1", "hello", "/workspace")

    with pytest.raises(TimeoutError, match="provider timeout"):
        _ = [
            chunk
            async for chunk in stream_native_chunks(
                RunTurnCoordinator(runtime), request, queue, on_event=lambda _: None
            )
        ]


@pytest.mark.asyncio
async def test_stream_native_chunks_cancels_runtime_if_consumer_stops_early():
    queue: asyncio.Queue[object] = asyncio.Queue(maxsize=1)

    class BlockingRuntime(FakeRuntime):
        async def stream_turn(self, request: TurnRequest) -> AsyncIterator[RunEvent]:
            del request
            self.started.set()
            await asyncio.Future()
            yield _event(1, RunEventKind.turn_finished)  # pragma: no cover

    runtime = BlockingRuntime(queue, [])
    coordinator = RunTurnCoordinator(runtime)
    request = TurnRequest("session-1", "turn-1", "hello", "/workspace")

    async def consume() -> None:
        async for _chunk in stream_native_chunks(
            coordinator, request, queue, on_event=lambda _: None
        ):
            pass

    task = asyncio.create_task(consume())
    await runtime.started.wait()
    task.cancel()
    with pytest.raises(asyncio.CancelledError):
        await task

    assert runtime.cancelled == [("session-1", "turn-1")]
