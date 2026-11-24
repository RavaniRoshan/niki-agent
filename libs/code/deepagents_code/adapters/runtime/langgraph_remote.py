"""Adapter from LangGraph remote streams to Niki runtime events."""

from __future__ import annotations

import hashlib
import os
from typing import TYPE_CHECKING, Protocol, cast

if TYPE_CHECKING:
    from collections.abc import (
        AsyncIterator,
        Awaitable,
        Callable,
        Iterator,
        Mapping,
        Sequence,
    )

from deepagents_code.application.run_turn import RunProtocolError
from deepagents_code.domain.events import (
    MAX_ENCODED_BYTES,
    SCHEMA_VERSION,
    GoalProgressEvent,
    GoalProgressStatus,
    ResumeRequest,
    RunEvent,
    RunEventKind,
    TurnRequest,
    encode_run_event,
)

ALLOWED_MODES = {"messages", "updates", "events", "custom"}
_STREAM_CHUNK_LENGTH = 3
_MESSAGE_CHUNK_LENGTH = 2
_TEXT_EVENT_CHARS = 16_000


class _RemoteAgent(Protocol):
    def astream(
        self,
        graph_input: object,
        *,
        stream_mode: Sequence[str],
        subgraphs: bool,
        config: Mapping[str, object],
        context: Mapping[str, object] | None = None,
    ) -> AsyncIterator[object]: ...


class LangGraphEventEncoder:
    """Encode allowlisted LangGraph observations into Niki RunEvents."""

    @staticmethod
    def encode(_namespace: tuple[str, ...], mode: str, data: object) -> RunEvent | None:
        """Validate and convert a public event envelope from the graph stream.

        Returns:
            A validated event, or `None` when the observation is not public.

        Raises:
            RunProtocolError: The public event envelope is malformed.
        """
        if mode not in ALLOWED_MODES:
            return None
        if not isinstance(data, dict):
            return None
        if "kind" not in data:
            return None
        data = cast("dict[str, object]", data)
        schema_version = data.get("schema_version", 1)
        session_id = data.get("session_id")
        turn_id = data.get("turn_id")
        sequence = data.get("sequence")
        kind_value = data.get("kind")
        payload = data.get("payload", {})
        if isinstance(schema_version, bool) or not isinstance(schema_version, int):
            msg = "malformed public event: schema_version must be int"
            raise RunProtocolError(msg)
        if not isinstance(session_id, str) or not isinstance(turn_id, str):
            msg = "malformed public event: session and turn IDs must be strings"
            raise RunProtocolError(msg)
        if isinstance(sequence, bool) or not isinstance(sequence, int) or sequence < 1:
            msg = "malformed public event: sequence must be a positive integer"
            raise RunProtocolError(msg)
        if not isinstance(payload, dict):
            msg = "malformed public event: payload must be an object"
            raise RunProtocolError(msg)
        try:
            kind = RunEventKind(kind_value)
        except (TypeError, ValueError) as exc:
            msg = f"malformed public event: invalid kind {kind_value!r}"
            raise RunProtocolError(msg) from exc
        from deepagents_code.domain.events import _validate_json_value

        try:
            validated_payload = _validate_json_value(payload)
        except ValueError as exc:
            msg = f"malformed public event: {exc}"
            raise RunProtocolError(msg) from exc
        if not isinstance(validated_payload, dict):
            msg = "malformed public event: payload must be an object"
            raise RunProtocolError(msg)
        return RunEvent(
            schema_version=schema_version,
            event_id=str(data.get("event_id", "")),
            session_id=session_id,
            turn_id=turn_id,
            sequence=sequence,
            kind=kind,
            payload=validated_payload,
        )


class LangGraphRemoteRuntime:
    """AgentRuntime backed by a managed LangGraph remote stream."""

    def __init__(
        self,
        remote_agent: _RemoteAgent,
        *,
        on_chunk: Callable[[object], Awaitable[None] | None] | None = None,
        stream_input: object | None = None,
        config: Mapping[str, object] | None = None,
        context: Mapping[str, object] | None = None,
        stream_mode: Sequence[str] = ("messages", "updates", "custom"),
    ) -> None:
        """Bind the remote graph and the presentation callback for native chunks."""
        self.remote_agent = remote_agent
        self.on_chunk = on_chunk
        self.stream_input = stream_input
        self.config = config
        self.context = context
        self.stream_mode = tuple(stream_mode)

    async def stream_turn(self, request: TurnRequest) -> AsyncIterator[RunEvent]:
        """Start one graph turn and emit normalized text and lifecycle events.

        Yields:
            Turn lifecycle, text, approval, and failure events.
        """
        graph_input = self.stream_input or {
            "messages": [{"role": "user", "content": request.prompt}]
        }
        async for event in self._stream(
            request.session_id,
            request.turn_id,
            graph_input,
            prefix=(_event(request, 1, RunEventKind.turn_started),),
            first_sequence=2,
        ):
            yield event

    async def resume_turn(self, request: ResumeRequest) -> AsyncIterator[RunEvent]:
        """Resume the checkpoint with the supplied decision and event sequence.

        Yields:
            Resume lifecycle, text, approval, and failure events.
        """
        from langgraph.types import Command

        sequence = request.last_sequence + 1
        prefix = (
            _event(request, sequence, RunEventKind.session_resumed),
            _event(request, sequence + 1, RunEventKind.turn_started),
        )
        async for event in self._stream(
            request.session_id,
            request.turn_id,
            Command(resume=request.decision),
            prefix=prefix,
            first_sequence=sequence + 2,
        ):
            yield event

    async def _stream(
        self,
        session_id: str,
        turn_id: str,
        graph_input: object,
        *,
        prefix: tuple[RunEvent, ...],
        first_sequence: int,
    ) -> AsyncIterator[RunEvent]:
        sequence = first_sequence
        approval_ids: list[str] = []
        for event in prefix:
            yield event
        try:
            async for chunk in self.remote_agent.astream(
                graph_input,
                stream_mode=list(self.stream_mode),
                subgraphs=True,
                config=self._config(session_id),
                context=self.context,
            ):
                if self.on_chunk is not None:
                    callback_result = self.on_chunk(chunk)
                    if callback_result is not None:
                        await callback_result
                projected = _custom_runtime_event(
                    chunk,
                    session_id=session_id,
                    turn_id=turn_id,
                    sequence=sequence,
                )
                if projected is not None:
                    yield projected
                    sequence = projected.sequence + 1
                tool_events = _tool_runtime_events(
                    chunk,
                    session_id=session_id,
                    turn_id=turn_id,
                    first_sequence=sequence,
                )
                for event in tool_events:
                    yield event
                if tool_events:
                    sequence = tool_events[-1].sequence + 1
                text = _message_text(chunk)
                if text:
                    for event in _text_events(session_id, turn_id, sequence, text):
                        yield event
                    sequence = event.sequence + 1
                approval_ids.extend(_interrupt_ids(chunk))
        except TimeoutError as exc:
            yield _failure(session_id, turn_id, sequence, exc, recoverable=True)
            return
        except Exception as exc:
            if isinstance(exc, RunProtocolError):
                raise
            yield _failure(session_id, turn_id, sequence, exc, recoverable=False)
            return

        if approval_ids:
            yield _event_for_ids(
                session_id,
                turn_id,
                sequence,
                RunEventKind.approval_required,
                {"interrupt_ids": approval_ids},
            )
        else:
            yield _event_for_ids(
                session_id, turn_id, sequence, RunEventKind.turn_finished
            )

    def _config(self, session_id: str) -> dict[str, object]:
        config = dict(self.config or {})
        configurable = dict(
            cast("Mapping[str, object]", config.get("configurable", {}))
        )
        configurable["thread_id"] = session_id
        config["configurable"] = configurable
        return config

    async def cancel_turn(self, session_id: str, turn_id: str) -> None:
        """Forward cancellation to the managed remote run when supported."""
        cancel = getattr(self.remote_agent, "acancel_active_runs", None)
        if callable(cancel):
            cancel_run = cast("Callable[[dict[str, object]], Awaitable[None]]", cancel)
            await cancel_run({"configurable": {"thread_id": session_id}})
        else:
            cancel_one = getattr(self.remote_agent, "cancel_turn", None)
            if callable(cancel_one):
                cancel_graph = cast("Callable[[str, str], Awaitable[None]]", cancel_one)
                await cancel_graph(session_id, turn_id)


def _message_text(chunk: object) -> str:
    if not isinstance(chunk, tuple) or len(chunk) != _STREAM_CHUNK_LENGTH:
        return ""
    namespace, mode, data = chunk
    if namespace or mode != "messages":
        return ""
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


_FILE_MUTATION_TOOLS = frozenset({"write_file", "edit_file", "delete"})


def _tool_runtime_events(
    chunk: object,
    *,
    session_id: str,
    turn_id: str,
    first_sequence: int,
) -> list[RunEvent]:
    """Project public tool lifecycle facts from LangGraph node updates.

    Only call identity, tool name, status, and a one-way file identifier leave
    the adapter. Tool arguments and outputs remain in the execution layer.

    Returns:
        Zero or more normalized tool lifecycle events in stream order.
    """
    if not isinstance(chunk, tuple) or len(chunk) != _STREAM_CHUNK_LENGTH:
        return []
    _namespace, mode, data = chunk
    if mode != "updates" or not isinstance(data, dict):
        return []

    node_updates = [data] if "messages" in data else list(data.values())
    events: list[RunEvent] = []
    sequence = first_sequence
    for update in node_updates:
        if not isinstance(update, dict):
            continue
        messages = update.get("messages")
        if not isinstance(messages, (list, tuple)):
            continue
        for message in messages:
            if getattr(message, "type", "") == "ai":
                calls = getattr(message, "tool_calls", ())
                if not isinstance(calls, (list, tuple)):
                    continue
                for call in calls:
                    if not isinstance(call, dict):
                        continue
                    call_id = call.get("id")
                    tool_name = call.get("name")
                    args = call.get("args")
                    if (
                        not isinstance(call_id, str)
                        or not call_id
                        or not isinstance(tool_name, str)
                        or not tool_name
                    ):
                        continue
                    payload: dict[str, object] = {
                        "call_id": call_id,
                        "tool_name": tool_name,
                    }
                    if tool_name in _FILE_MUTATION_TOOLS and isinstance(args, dict):
                        path = args.get("file_path")
                        if isinstance(path, str) and path:
                            normalized_path = os.path.normcase(os.path.normpath(path))
                            payload["file_id"] = hashlib.sha256(
                                normalized_path.encode("utf-8")
                            ).hexdigest()
                    events.append(
                        _event_for_ids(
                            session_id,
                            turn_id,
                            sequence,
                            RunEventKind.tool_started,
                            payload,
                        )
                    )
                    sequence += 1
            elif getattr(message, "type", "") == "tool":
                call_id = getattr(message, "tool_call_id", None)
                tool_name = getattr(message, "name", None)
                status = getattr(message, "status", None)
                if not isinstance(call_id, str) or not call_id:
                    continue
                payload = {"call_id": call_id}
                if isinstance(tool_name, str) and tool_name:
                    payload["tool_name"] = tool_name
                if isinstance(status, str) and status:
                    payload["status"] = status
                events.append(
                    _event_for_ids(
                        session_id,
                        turn_id,
                        sequence,
                        RunEventKind.tool_finished,
                        payload,
                    )
                )
                sequence += 1
    return events


def _custom_runtime_event(
    chunk: object,
    *,
    session_id: str,
    turn_id: str,
    sequence: int,
) -> RunEvent | None:
    """Normalize allowlisted custom lifecycle observations for one turn.

    Returns:
        A run event when the chunk is a recognized custom lifecycle event.

    Raises:
        RunProtocolError: If a recognized event has an invalid payload.
    """
    if not isinstance(chunk, tuple) or len(chunk) != _STREAM_CHUNK_LENGTH:
        return None
    namespace, mode, data = chunk
    if namespace or mode != "custom" or not isinstance(data, dict):
        return None
    event_type = data.get("type")
    payload: dict[str, object]
    if event_type == "niki_goal_progress":
        status = data.get("status")
        note = data.get("note", "")
        if not isinstance(status, str) or not isinstance(note, str):
            msg = "malformed goal progress event: status and note must be strings"
            raise RunProtocolError(msg)
        try:
            progress = GoalProgressEvent(
                session_id=session_id,
                status=cast("GoalProgressStatus", status),
                note=note,
            )
        except (TypeError, ValueError) as exc:
            msg = f"malformed goal progress event: {exc}"
            raise RunProtocolError(msg) from exc
        if progress.status == "complete" and data.get("verified") is not True:
            msg = "malformed goal progress event: complete status requires verification"
            raise RunProtocolError(msg)
        kind = RunEventKind.goal_progress
        payload = {"status": progress.status, "note": progress.note}
        if progress.status == "complete":
            payload["verified"] = True
    elif event_type == "niki_context_compacted":
        cutoff_index = data.get("cutoff_index")
        if (
            isinstance(cutoff_index, bool)
            or not isinstance(cutoff_index, int)
            or cutoff_index < 0
        ):
            msg = "malformed context compaction event: invalid cutoff_index"
            raise RunProtocolError(msg)
        kind = RunEventKind.context_compacted
        payload = {"cutoff_index": cutoff_index}
    else:
        return None
    return RunEvent(
        schema_version=SCHEMA_VERSION,
        event_id=f"{turn_id}:{sequence}",
        session_id=session_id,
        turn_id=turn_id,
        sequence=sequence,
        kind=kind,
        payload=payload,
    )


def _interrupt_ids(chunk: object) -> list[str]:
    if not isinstance(chunk, tuple) or len(chunk) != _STREAM_CHUNK_LENGTH:
        return []
    _namespace, mode, data = chunk
    if mode != "updates" or not isinstance(data, dict):
        return []
    interrupts = data.get("__interrupt__")
    if not isinstance(interrupts, (list, tuple)):
        return []
    return [
        interrupt_id for item in interrupts if (interrupt_id := _interrupt_id(item))
    ]


def _interrupt_id(value: object) -> str:
    value = value.get("id") if isinstance(value, dict) else getattr(value, "id", None)
    return value if isinstance(value, str) else ""


def _event(
    request: TurnRequest | ResumeRequest,
    sequence: int,
    kind: RunEventKind,
) -> RunEvent:
    return _event_for_ids(request.session_id, request.turn_id, sequence, kind)


def _event_for_ids(
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
    return _event_for_ids(
        session_id,
        turn_id,
        sequence,
        RunEventKind.turn_failed,
        {"error": str(error), "recoverable": recoverable},
    )


def _text_events(
    session_id: str,
    turn_id: str,
    first_sequence: int,
    text: str,
) -> Iterator[RunEvent]:
    sequence = first_sequence
    offset = 0
    while offset < len(text):
        size = min(_TEXT_EVENT_CHARS, len(text) - offset)
        while size > 0:
            event = _event_for_ids(
                session_id,
                turn_id,
                sequence,
                RunEventKind.text_delta,
                {"text": text[offset : offset + size]},
            )
            try:
                encode_run_event(event)
            except ValueError:
                size //= 2
                continue
            yield event
            offset += size
            sequence += 1
            break
        if size == 0:
            msg = f"text event exceeds the {MAX_ENCODED_BYTES}-byte limit"
            raise RunProtocolError(msg)
