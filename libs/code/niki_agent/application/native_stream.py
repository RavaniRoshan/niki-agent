"""Bridge validated runtime events back to a surface's native observations."""

from __future__ import annotations

import asyncio
from contextlib import suppress
from typing import TYPE_CHECKING

from niki_agent.domain.events import (
    ResumeRequest,
    RunEvent,
    RunEventKind,
    TurnRequest,
)

if TYPE_CHECKING:
    from collections.abc import AsyncIterator, Callable

    from niki_agent.application.run_turn import RunTurnCoordinator

_STREAM_END = object()


async def stream_native_chunks(
    coordinator: RunTurnCoordinator,
    request: TurnRequest | ResumeRequest,
    chunks: asyncio.Queue[object],
    *,
    on_event: Callable[[RunEvent], None],
) -> AsyncIterator[object]:
    """Yield native observations while validating the matching runtime events.

    The queue is bounded by the caller to apply backpressure to the graph
    stream. Closing this iterator early cancels the owned runtime turn.

    Raises:
        CancelledError: The runtime reports a cancelled turn or the consumer stops.
    """
    task = asyncio.create_task(_produce_events(coordinator, request, chunks, on_event))
    try:
        async for chunk in _queued_chunks(chunks):
            yield chunk
        if await task:
            raise asyncio.CancelledError
    finally:
        if not task.done():
            task.cancel()
            with suppress(asyncio.CancelledError):
                await task
            await coordinator.cancel_turn(request.session_id, request.turn_id)


async def _queued_chunks(chunks: asyncio.Queue[object]) -> AsyncIterator[object]:
    while True:
        chunk = await chunks.get()
        if chunk is _STREAM_END:
            return
        yield chunk


async def _produce_events(
    coordinator: RunTurnCoordinator,
    request: TurnRequest | ResumeRequest,
    chunks: asyncio.Queue[object],
    on_event: Callable[[RunEvent], None],
) -> bool:
    cancelled = False
    try:
        async for event in _turn_events(coordinator, request):
            on_event(event)
            if event.kind == RunEventKind.turn_failed:
                _raise_runtime_failure(event)
            if event.kind == RunEventKind.turn_cancelled:
                cancelled = True
                break
    finally:
        task = asyncio.current_task()
        if task is None or task.cancelling() == 0:
            await chunks.put(_STREAM_END)
    return cancelled


def _turn_events(
    coordinator: RunTurnCoordinator,
    request: TurnRequest | ResumeRequest,
) -> AsyncIterator[RunEvent]:
    if isinstance(request, ResumeRequest):
        return coordinator.resume_turn(request)
    return coordinator.stream_turn(request)


def _raise_runtime_failure(event: RunEvent) -> None:
    message = str(event.payload.get("error", "Agent runtime failed"))
    if event.payload.get("recoverable") is True:
        raise TimeoutError(message)
    raise RuntimeError(message)
