"""Tests for Niki entry-point runtime routing (interface-level)."""

from __future__ import annotations

import asyncio
from typing import TYPE_CHECKING

import pytest
from langchain_core.messages import AIMessageChunk
from langgraph.types import Command

from niki_agent.adapters.runtime.acp import ACPAgentRuntime
from niki_agent.application.run_turn import RunTurnCoordinator
from niki_agent.domain.events import (
    ResumeRequest,
    RunEvent,
    RunEventKind,
    TurnRequest,
    TurnStatus,
)
from niki_agent.niki.runtime_mode import (
    is_niki_runtime_enabled,
    niki_runtime_scope,
)

if TYPE_CHECKING:
    from collections.abc import AsyncIterator, Mapping, Sequence


class FakeAgent:
    def __init__(self, streams: list[list[object]]) -> None:
        self.streams = streams
        self.calls: list[tuple[object, dict[str, object]]] = []
        self.tool_executions = 0

    async def astream(
        self,
        graph_input: object,
        *,
        config: Mapping[str, object],
        stream_mode: Sequence[str],
        subgraphs: bool,
        **_kwargs: object,
    ) -> AsyncIterator[object]:
        self.calls.append((graph_input, dict(config)))
        assert stream_mode == ["messages", "updates", "custom"]
        assert subgraphs is True
        if not isinstance(graph_input, Command):
            self.tool_executions += 1
        for chunk in self.streams[len(self.calls) - 1]:
            yield chunk


def test_niki_runtime_scope_is_process_context_only():
    assert not is_niki_runtime_enabled()

    with niki_runtime_scope():
        assert is_niki_runtime_enabled()

    assert not is_niki_runtime_enabled()


@pytest.mark.asyncio
async def test_acp_runtime_maps_session_events_and_approval_resume():
    interrupt = {"__interrupt__": [{"id": "i1", "value": {"action_requests": []}}]}
    agent = FakeAgent([[((), "updates", interrupt)], []])
    rt = ACPAgentRuntime(agent)
    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")
    coordinator = RunTurnCoordinator(rt)
    out = [event async for event in coordinator.stream_turn(req)]
    assert [event.kind for event in out] == [
        RunEventKind.turn_started,
        RunEventKind.approval_required,
    ]

    resume = ResumeRequest(
        session_id="s1",
        turn_id="t1",
        decision={"decisions": [{"type": "approve"}]},
        last_sequence=2,
        prior_status=TurnStatus.waiting_for_approval,
    )
    resumed = [event async for event in coordinator.resume_turn(resume)]
    assert [event.kind for event in resumed] == [
        RunEventKind.session_resumed,
        RunEventKind.turn_started,
        RunEventKind.turn_finished,
    ]
    assert [event.sequence for event in resumed] == [3, 4, 5]
    assert isinstance(agent.calls[1][0], Command)
    assert agent.calls[1][0].resume == resume.decision
    assert agent.calls[1][1] == {"configurable": {"thread_id": "s1"}}


@pytest.mark.asyncio
async def test_acp_runtime_maps_streamed_text_to_run_event():
    agent = FakeAgent([[((), "messages", (AIMessageChunk(content="hello"), {}))]])
    rt = ACPAgentRuntime(agent)
    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")

    out = [event async for event in rt.stream_turn(req)]

    assert [event.kind for event in out] == [
        RunEventKind.turn_started,
        RunEventKind.text_delta,
        RunEventKind.turn_finished,
    ]
    assert out[1].payload == {"text": "hello"}
    assert agent.calls[0][0] == {"messages": [{"role": "user", "content": "hi"}]}


@pytest.mark.asyncio
async def test_acp_runtime_forwards_native_input_and_chunks():
    chunks = [
        ((), "messages", (AIMessageChunk(content="hello"), {})),
        (
            (),
            "updates",
            {"tools": {"todos": [{"content": "inspect", "status": "pending"}]}},
        ),
    ]
    native_input = {
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "inspect"},
                    {"type": "image", "base64": "AA=="},
                ],
            }
        ]
    }
    context = object()

    class ConfiguredAgent:
        def __init__(self) -> None:
            self.calls: list[
                tuple[object, dict[str, object], list[str], bool, dict[str, object]]
            ] = []

        async def astream(
            self,
            graph_input: object,
            *,
            config: Mapping[str, object],
            stream_mode: Sequence[str],
            subgraphs: bool,
            **kwargs: object,
        ) -> AsyncIterator[object]:
            self.calls.append(
                (graph_input, dict(config), list(stream_mode), subgraphs, kwargs)
            )
            for chunk in chunks:
                yield chunk

    agent = ConfiguredAgent()
    received: list[object] = []
    config = {
        "configurable": {"thread_id": "acp-session"},
        "metadata": {"cwd": "/workspace"},
    }
    runtime = ACPAgentRuntime(
        agent,
        on_chunk=received.append,
        stream_input=native_input,
        config=config,
        stream_mode=("messages", "updates"),
        stream_kwargs={"context": context},
    )
    request = TurnRequest("acp-session", "turn-1", "inspect", "/workspace")

    events = [event async for event in RunTurnCoordinator(runtime).stream_turn(request)]

    assert agent.calls == [
        (native_input, config, ["messages", "updates"], True, {"context": context})
    ]
    assert received == chunks
    assert [event.kind for event in events] == [
        RunEventKind.turn_started,
        RunEventKind.text_delta,
        RunEventKind.turn_finished,
    ]


@pytest.mark.asyncio
async def test_acp_runtime_resumes_with_server_config_and_sequence():
    interrupt = {"__interrupt__": [{"id": "approval-1"}]}

    class ConfiguredAgent:
        def __init__(self) -> None:
            self.calls: list[tuple[object, dict[str, object]]] = []

        async def astream(
            self,
            graph_input: object,
            *,
            config: Mapping[str, object],
            stream_mode: Sequence[str],
            subgraphs: bool,
            **_kwargs: object,
        ) -> AsyncIterator[object]:
            assert list(stream_mode) == ["messages", "updates"]
            assert subgraphs is True
            self.calls.append((graph_input, dict(config)))
            yield ((), "updates", interrupt) if len(self.calls) == 1 else None

    agent = ConfiguredAgent()
    config = {
        "configurable": {"thread_id": "acp-session"},
        "metadata": {"cwd": "/workspace", "acp_mode": "auto"},
    }
    runtime = ACPAgentRuntime(
        agent,
        stream_input={"messages": [{"role": "user", "content": "review"}]},
        config=config,
        stream_mode=("messages", "updates"),
    )
    coordinator = RunTurnCoordinator(runtime)
    request = TurnRequest("acp-session", "turn-1", "review", "/workspace")
    first = [event async for event in coordinator.stream_turn(request)]
    resume = ResumeRequest(
        "acp-session",
        "turn-1",
        {"decisions": [{"type": "approve"}]},
        first[-1].sequence,
        TurnStatus.waiting_for_approval,
    )

    resumed = [event async for event in coordinator.resume_turn(resume)]

    assert [event.sequence for event in resumed] == [3, 4, 5]
    assert [event.kind for event in resumed] == [
        RunEventKind.session_resumed,
        RunEventKind.turn_started,
        RunEventKind.turn_finished,
    ]
    graph_input, graph_config = agent.calls[1]
    assert isinstance(graph_input, Command)
    assert graph_input.resume == resume.decision
    assert graph_config == config


@pytest.mark.asyncio
async def test_interrupted_turn_resumes_without_repeating_completed_tool():
    agent = FakeAgent([[], []])
    rt = ACPAgentRuntime(agent)
    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")
    first = [event async for event in rt.stream_turn(req)]
    resume = ResumeRequest(
        session_id="s1",
        turn_id="t1",
        decision={"decisions": [{"type": "approve"}]},
        last_sequence=first[-1].sequence,
        prior_status=TurnStatus.running,
    )

    resumed = [event async for event in rt.resume_turn(resume)]

    assert agent.calls[0][0] == {"messages": [{"role": "user", "content": "hi"}]}
    assert isinstance(agent.calls[1][0], Command)
    assert agent.calls[1][0].resume == resume.decision
    assert agent.tool_executions == 1
    assert resumed[-1].kind == RunEventKind.turn_finished


@pytest.mark.asyncio
async def test_acp_runtime_cancel_stops_owned_graph_stream():
    started = asyncio.Event()

    class BlockingAgent:
        async def astream(
            self,
            graph_input: object,
            *,
            config: Mapping[str, object],
            stream_mode: Sequence[str],
            subgraphs: bool,
            **_kwargs: object,
        ) -> AsyncIterator[object]:
            assert graph_input == {"messages": [{"role": "user", "content": "hi"}]}
            assert config == {"configurable": {"thread_id": "s1"}}
            assert stream_mode == ["messages", "updates", "custom"]
            assert subgraphs is True
            started.set()
            await asyncio.Future()
            yield None  # pragma: no cover

    runtime = ACPAgentRuntime(BlockingAgent())
    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")
    task = asyncio.create_task(_collect_events(runtime, req))
    await started.wait()

    await runtime.cancel_turn("s1", "t1")

    events = await task
    assert events[-1].kind == RunEventKind.turn_cancelled
    assert not runtime._active_tasks


async def _collect_events(runtime: ACPAgentRuntime, req: TurnRequest) -> list[RunEvent]:
    return [event async for event in RunTurnCoordinator(runtime).stream_turn(req)]


@pytest.mark.asyncio
async def test_provider_timeout_emits_recoverable_failure():
    class FailingAgent:
        async def astream(
            self,
            graph_input: object,
            *,
            config: Mapping[str, object],
            stream_mode: Sequence[str],
            subgraphs: bool,
            **_kwargs: object,
        ) -> AsyncIterator[object]:
            assert graph_input == {"messages": [{"role": "user", "content": "hi"}]}
            assert config == {"configurable": {"thread_id": "s1"}}
            assert stream_mode == ["messages", "updates", "custom"]
            assert subgraphs is True
            yield ((), "messages", (AIMessageChunk(content="partial"), {}))
            msg = "provider timeout"
            raise TimeoutError(msg)

    rt = ACPAgentRuntime(FailingAgent())
    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")
    out = [event async for event in RunTurnCoordinator(rt).stream_turn(req)]
    assert [event.sequence for event in out] == [1, 2, 3]
    assert out[-1].kind == RunEventKind.turn_failed
    assert out[-1].payload == {
        "error": "provider timeout",
        "recoverable": True,
    }
