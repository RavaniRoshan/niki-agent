"""Adapter from a checkpointed LangGraph graph to Niki run events."""

from __future__ import annotations

import asyncio
from typing import TYPE_CHECKING, Protocol, cast

if TYPE_CHECKING:
    from collections.abc import AsyncIterator, Awaitable, Callable, Mapping, Sequence

from deepagents_code.domain.events import (
    ResumeRequest,
    RunEvent,
    RunEventKind,
    TurnRequest,
)


class _PregelGraph(Protocol):
    def astream(
        self,
        graph_input: object,
        *,
        config: Mapping[str, object],
        stream_mode: Sequence[str],
        subgraphs: bool,
    ) -> AsyncIterator[object]:
        ...


_STREAM_CHUNK_LENGTH = 3
_MESSAGE_CHUNK_LENGTH = 2


class ACPAgentRuntime:
    """Run and resume a graph using the thread checkpoint as its state."""

    def __init__(self, agent: _PregelGraph) -> None:
        """Bind the compiled graph whose checkpoints own turn state."""
        self.agent = agent
        self._active_tasks: dict[tuple[str, str], asyncio.Task[object]] = {}
        self._cancel_requested: set[tuple[str, str]] = set()

    async def stream_turn(self, request: TurnRequest) -> AsyncIterator[RunEvent]:
        """Execute a prompt and emit its text and terminal lifecycle event.

        Yields:
            Lifecycle, assistant text, approval, and terminal events.
        """
        task_key = (request.session_id, request.turn_id)
        self._record_active_task(task_key)
        yield _event(request.session_id, request.turn_id, 1, RunEventKind.turn_started)
        try:
            async for event in self._stream_graph(
                request.session_id,
                request.turn_id,
                {"messages": [{"role": "user", "content": request.prompt}]},
                first_sequence=2,
                task_key=task_key,
            ):
                yield event
        finally:
            self._active_tasks.pop(task_key, None)

    async def resume_turn(self, request: ResumeRequest) -> AsyncIterator[RunEvent]:
        """Resume the same checkpoint with the supplied approval decision.

        Yields:
            Resume, assistant text, approval, and terminal events.
        """
        task_key = (request.session_id, request.turn_id)
        self._record_active_task(task_key)
        sequence = request.last_sequence + 1
        yield _event(
            request.session_id,
            request.turn_id,
            sequence,
            RunEventKind.session_resumed,
        )
        yield _event(
            request.session_id,
            request.turn_id,
            sequence + 1,
            RunEventKind.turn_started,
        )
        try:
            from langgraph.types import Command

            async for event in self._stream_graph(
                request.session_id,
                request.turn_id,
                Command(resume=request.decision),
                first_sequence=sequence + 2,
                task_key=task_key,
            ):
                yield event
        finally:
            self._active_tasks.pop(task_key, None)

    async def cancel_turn(self, session_id: str, turn_id: str) -> None:
        """Cancel the graph run owned by this session and turn."""
        task_key = (session_id, turn_id)
        task = self._active_tasks.get(task_key)
        if task is None or task is asyncio.current_task():
            return
        self._cancel_requested.add(task_key)
        cancel = getattr(self.agent, "acancel_active_runs", None)
        if callable(cancel):
            cancel_graph = cast(
                "Callable[[dict[str, object]], Awaitable[None]]", cancel
            )
            await cancel_graph({"configurable": {"thread_id": session_id}})
            return
        task.cancel()

    def _record_active_task(self, task_key: tuple[str, str]) -> None:
        task = asyncio.current_task()
        if task is not None:
            self._active_tasks[task_key] = cast("asyncio.Task[object]", task)

    async def _stream_graph(
        self,
        session_id: str,
        turn_id: str,
        graph_input: object,
        *,
        first_sequence: int,
        task_key: tuple[str, str],
    ) -> AsyncIterator[RunEvent]:
        sequence = first_sequence
        approval_ids: list[str] = []
        try:
            async for chunk in self.agent.astream(
                graph_input,
                config={"configurable": {"thread_id": session_id}},
                stream_mode=["messages", "updates", "custom"],
                subgraphs=True,
            ):
                if not isinstance(chunk, tuple) or len(chunk) != _STREAM_CHUNK_LENGTH:
                    continue
                namespace, mode, data = chunk
                if mode == "messages" and not namespace:
                    text = _message_text(data)
                    if text:
                        yield _event(
                            session_id,
                            turn_id,
                            sequence,
                            RunEventKind.text_delta,
                            {"text": text},
                        )
                        sequence += 1
                elif mode == "updates" and isinstance(data, dict):
                    interrupts = data.get("__interrupt__")
                    if isinstance(interrupts, (list, tuple)):
                        approval_ids.extend(_interrupt_id(item) for item in interrupts)
        except asyncio.CancelledError:
            if task_key not in self._cancel_requested:
                raise
            self._cancel_requested.discard(task_key)
            yield _event(session_id, turn_id, sequence, RunEventKind.turn_cancelled)
            return
        except TimeoutError as exc:
            yield _failure(session_id, turn_id, sequence, exc, recoverable=True)
            return
        except Exception as exc:  # noqa: BLE001 - graph errors become terminal run events
            yield _failure(session_id, turn_id, sequence, exc, recoverable=False)
            return

        if task_key in self._cancel_requested:
            self._cancel_requested.discard(task_key)
            yield _event(session_id, turn_id, sequence, RunEventKind.turn_cancelled)
            return
        if approval_ids:
            yield _event(
                session_id,
                turn_id,
                sequence,
                RunEventKind.approval_required,
                {"interrupt_ids": approval_ids},
            )
        else:
            yield _event(session_id, turn_id, sequence, RunEventKind.turn_finished)


def _message_text(data: object) -> str:
    if not isinstance(data, tuple) or len(data) != _MESSAGE_CHUNK_LENGTH:
        return ""
    message = data[0]
    if getattr(message, "type", "") not in {"ai", "AIMessageChunk"}:
        return ""
    content = getattr(message, "content", None)
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return "".join(
            block["text"]
            for block in content
            if isinstance(block, dict)
            and block.get("type") == "text"
            and isinstance(block.get("text"), str)
        )
    return ""


def _interrupt_id(interrupt: object) -> str:
    value = getattr(interrupt, "id", None)
    if isinstance(value, str):
        return value
    if isinstance(interrupt, dict):
        value = interrupt.get("id")
        if isinstance(value, str):
            return value
    return ""


def _event(
    session_id: str,
    turn_id: str,
    sequence: int,
    kind: RunEventKind,
    payload: dict[str, object] | None = None,
) -> RunEvent:
    return RunEvent(
        schema_version=1,
        event_id=f"{turn_id}:{sequence}",
        session_id=session_id,
        turn_id=turn_id,
        sequence=sequence,
        kind=kind,
        payload=payload or {},
    )


def _failure(
    session_id: str,
    turn_id: str,
    sequence: int,
    error: Exception,
    *,
    recoverable: bool,
) -> RunEvent:
    return _event(
        session_id,
        turn_id,
        sequence,
        RunEventKind.turn_failed,
        {"error": str(error), "recoverable": recoverable},
    )
