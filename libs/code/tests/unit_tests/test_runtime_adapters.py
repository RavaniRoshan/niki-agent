"""Tests for the LangGraph runtime adapter."""

from __future__ import annotations

from typing import TYPE_CHECKING, cast

import pytest
from langchain_core.messages import AIMessage, AIMessageChunk, ToolMessage
from langgraph.types import Command

from niki_agent.adapters.runtime.langgraph_remote import (
    LangGraphEventEncoder,
    LangGraphRemoteRuntime,
)
from niki_agent.application.run_turn import RunProtocolError, RunTurnCoordinator
from niki_agent.domain.events import (
    JsonObject,
    ResumeRequest,
    RunEvent,
    RunEventKind,
    TurnRequest,
    TurnStatus,
    encode_run_event,
)

if TYPE_CHECKING:
    from collections.abc import AsyncIterator, Mapping, Sequence


class FakeRemoteAgent:
    def __init__(self, events: Sequence[object]) -> None:
        self._events = events
        self.cancelled: list[Mapping[str, object]] = []
        self.calls: list[
            tuple[object, Sequence[str], bool, Mapping[str, object], object]
        ] = []

    async def astream(
        self,
        graph_input: object,
        *,
        stream_mode: Sequence[str],
        subgraphs: bool,
        config: Mapping[str, object],
        context: Mapping[str, object] | None = None,
    ) -> AsyncIterator[object]:
        self.calls.append((graph_input, stream_mode, subgraphs, config, context))
        for event in self._events:
            yield event

    async def acancel_active_runs(self, config: Mapping[str, object]) -> None:
        self.cancelled.append(config)


@pytest.mark.asyncio
async def test_remote_runtime_delivers_native_chunks_and_approval_lifecycle():
    chunks = [
        ((), "messages", (AIMessageChunk(content="hello"), {})),
        ((), "updates", {"__interrupt__": [{"id": "approval-1"}]}),
    ]
    received: list[object] = []

    def receive(chunk: object) -> None:
        received.append(chunk)

    runtime = LangGraphRemoteRuntime(FakeRemoteAgent(chunks), on_chunk=receive)
    request = TurnRequest("session-1", "turn-1", "hello", "/workspace")

    events = [event async for event in RunTurnCoordinator(runtime).stream_turn(request)]

    assert [event.kind for event in events] == [
        RunEventKind.turn_started,
        RunEventKind.text_delta,
        RunEventKind.approval_required,
    ]
    assert events[1].payload == {"text": "hello"}
    assert events[-1].payload == {"interrupt_ids": ["approval-1"]}
    assert received == chunks


@pytest.mark.asyncio
async def test_remote_runtime_projects_goal_and_compaction_custom_events():
    chunks = [
        (
            (),
            "custom",
            {
                "type": "niki_goal_progress",
                "status": "blocked",
                "note": "need API credentials",
            },
        ),
        ((), "custom", {"type": "niki_context_compacted", "cutoff_index": 7}),
    ]
    runtime = LangGraphRemoteRuntime(FakeRemoteAgent(chunks))
    request = TurnRequest("session-1", "turn-1", "continue", "/workspace")

    events = [event async for event in RunTurnCoordinator(runtime).stream_turn(request)]

    assert [event.kind for event in events] == [
        RunEventKind.turn_started,
        RunEventKind.goal_progress,
        RunEventKind.context_compacted,
        RunEventKind.turn_finished,
    ]
    assert [event.sequence for event in events] == [1, 2, 3, 4]
    assert events[1].payload == {"status": "blocked", "note": "need API credentials"}
    assert events[2].payload == {"cutoff_index": 7}


@pytest.mark.asyncio
async def test_remote_runtime_projects_tool_calls_and_results():
    chunks = [
        (
            (),
            "updates",
            {
                "model": {
                    "messages": [
                        AIMessage(
                            content="",
                            tool_calls=[
                                {
                                    "name": "write_file",
                                    "args": {"file_path": "src/agent.py"},
                                    "id": "call-write",
                                    "type": "tool_call",
                                }
                            ],
                        )
                    ]
                }
            },
        ),
        (
            (),
            "updates",
            {
                "tools": {
                    "messages": [
                        ToolMessage(
                            content="written",
                            name="write_file",
                            tool_call_id="call-write",
                            status="success",
                        )
                    ]
                }
            },
        ),
    ]
    runtime = LangGraphRemoteRuntime(FakeRemoteAgent(chunks))
    request = TurnRequest("session-1", "turn-1", "write a file", "/workspace")

    events = [event async for event in RunTurnCoordinator(runtime).stream_turn(request)]

    assert [event.kind for event in events] == [
        RunEventKind.turn_started,
        RunEventKind.tool_started,
        RunEventKind.tool_finished,
        RunEventKind.turn_finished,
    ]
    started, finished = events[1:3]
    assert started.payload == {
        "call_id": "call-write",
        "tool_name": "write_file",
        "file_id": started.payload["file_id"],
    }
    file_id = started.payload["file_id"]
    assert isinstance(file_id, str)
    assert len(file_id) == 64
    assert "src/agent.py" not in str(started.payload)
    assert finished.payload == {
        "call_id": "call-write",
        "tool_name": "write_file",
        "status": "success",
    }


@pytest.mark.asyncio
async def test_remote_runtime_rejects_invalid_goal_progress_status():
    chunks = [
        ((), "custom", {"type": "niki_goal_progress", "status": "finished", "note": ""})
    ]
    runtime = LangGraphRemoteRuntime(FakeRemoteAgent(chunks))
    request = TurnRequest("session-1", "turn-1", "continue", "/workspace")

    with pytest.raises(RunProtocolError, match="goal progress"):
        _ = [event async for event in RunTurnCoordinator(runtime).stream_turn(request)]


@pytest.mark.asyncio
async def test_remote_runtime_rejects_unverified_goal_completion_event():
    chunks = [
        (
            (),
            "custom",
            {"type": "niki_goal_progress", "status": "complete", "note": "done"},
        )
    ]
    runtime = LangGraphRemoteRuntime(FakeRemoteAgent(chunks))
    request = TurnRequest("session-1", "turn-1", "continue", "/workspace")

    with pytest.raises(RunProtocolError, match="requires verification"):
        _ = [event async for event in RunTurnCoordinator(runtime).stream_turn(request)]


@pytest.mark.asyncio
async def test_remote_runtime_splits_large_utf8_text_events():
    text = "🔥" * 20_000
    agent = FakeRemoteAgent([((), "messages", (AIMessageChunk(content=text), {}))])
    runtime = LangGraphRemoteRuntime(agent)
    request = TurnRequest("session-1", "turn-1", "write", "/workspace")

    events = [event async for event in RunTurnCoordinator(runtime).stream_turn(request)]
    text_events = [event for event in events if event.kind == RunEventKind.text_delta]

    assert len(text_events) > 1
    text_parts = [cast("str", event.payload["text"]) for event in text_events]
    assert "".join(text_parts) == text
    for event in events:
        encode_run_event(event)


@pytest.mark.asyncio
async def test_remote_runtime_resumes_with_approval_and_continued_sequence():
    decision: JsonObject = {"decisions": [{"type": "approve"}]}
    agent = FakeRemoteAgent([])
    runtime = LangGraphRemoteRuntime(agent)
    request = ResumeRequest(
        session_id="session-1",
        turn_id="turn-1",
        decision=decision,
        last_sequence=2,
        prior_status=TurnStatus.waiting_for_approval,
    )

    events = [event async for event in RunTurnCoordinator(runtime).resume_turn(request)]

    assert [event.kind for event in events] == [
        RunEventKind.session_resumed,
        RunEventKind.turn_started,
        RunEventKind.turn_finished,
    ]
    assert [event.sequence for event in events] == [3, 4, 5]
    graph_input, _, _, config, _ = agent.calls[0]
    assert isinstance(graph_input, Command)
    assert graph_input.resume == decision
    configurable = config.get("configurable")
    assert isinstance(configurable, dict)
    assert configurable.get("thread_id") == "session-1"


def _ev(
    seq: int,
    kind: str,
    sid: str = "s1",
    tid: str = "t1",
) -> tuple[tuple[str, ...], str, dict[str, object]]:
    return (
        (),
        "events",
        {
            "session_id": sid,
            "turn_id": tid,
            "sequence": seq,
            "kind": kind,
            "payload": {},
        },
    )


@pytest.mark.asyncio
async def test_remote_runtime_forwards_unrecognized_chunks_to_presentation_callback():
    events = [
        ((), "messages", {"foo": "bar"}),
        _ev(1, "turn_started"),
        _ev(2, "turn_finished"),
    ]
    received: list[object] = []

    def receive(chunk: object) -> None:
        received.append(chunk)

    rt = LangGraphRemoteRuntime(FakeRemoteAgent(events), on_chunk=receive)

    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")
    out = [e async for e in rt.stream_turn(req)]
    assert [event.kind for event in out] == [
        RunEventKind.turn_started,
        RunEventKind.turn_finished,
    ]
    assert received == events


def test_remote_runtime_rejects_malformed_public_event():
    event = {
        "session_id": "s1",
        "turn_id": "t1",
        "sequence": 1,
        "kind": "text_delta",
        "payload": "not-a-dict",
    }
    enc = LangGraphEventEncoder()
    with pytest.raises(RunProtocolError):
        enc.encode((), "events", event)


@pytest.mark.asyncio
async def test_remote_runtime_forwards_cancel_to_owned_run():
    events = []
    agent = FakeRemoteAgent(events)
    rt = LangGraphRemoteRuntime(agent)
    await rt.cancel_turn("s1", "t1")
    assert len(agent.cancelled) == 1


def test_encoder_returns_none_for_unsupported_mode():
    enc = LangGraphEventEncoder()
    assert enc.encode((), "internal", {"kind": "text_delta"}) is None


def test_encoder_returns_none_for_internal_observation():
    enc = LangGraphEventEncoder()
    assert enc.encode((), "messages", {"foo": "bar"}) is None
