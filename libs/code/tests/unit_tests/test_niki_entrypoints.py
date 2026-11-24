"""Tests for Niki entry-point runtime routing (interface-level)."""

from __future__ import annotations

import asyncio
from typing import TYPE_CHECKING

import pytest
from langchain_core.messages import AIMessageChunk
from langgraph.types import Command

from deepagents_code.adapters.runtime.acp import ACPAgentRuntime
from deepagents_code.application.run_turn import RunTurnCoordinator
from deepagents_code.domain.events import (
    ResumeRequest,
    RunEvent,
    RunEventKind,
    TurnRequest,
    TurnStatus,
)
from deepagents_code.niki.runtime_mode import (
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
async def test_cancel_turn_cancels_owned_graph_stream():
    started = asyncio.Event()

    class BlockingAgent:
        async def astream(
            self,
            graph_input: object,
            *,
            config: Mapping[str, object],
            stream_mode: Sequence[str],
            subgraphs: bool,
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
