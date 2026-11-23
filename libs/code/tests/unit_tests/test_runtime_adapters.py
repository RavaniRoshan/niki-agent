"""Tests for the LangGraph runtime adapter."""

from __future__ import annotations

from collections.abc import AsyncIterator

import pytest

from deepagents_code.adapters.runtime.langgraph_remote import (
    LangGraphEventEncoder,
    LangGraphRemoteRuntime,
)
from deepagents_code.application.run_turn import RunProtocolError
from deepagents_code.domain.events import RunEvent, RunEventKind


class FakeRemoteAgent:
    def __init__(self, events):
        self._events = events
        self.cancelled = []

    async def astream(self, *a, **kw):
        for e in self._events:
            yield e

    async def acancel_active_runs(self, config):
        self.cancelled.append(config)


def _ev(seq, kind, sid="s1", tid="t1"):
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
async def test_remote_runtime_maps_text_tool_approval_and_terminal_events():
    events = [
        _ev(1, "turn_started"),
        _ev(2, "text_delta"),
        _ev(3, "tool_started"),
        _ev(4, "tool_finished"),
        _ev(5, "approval_required"),
        _ev(6, "turn_finished"),
    ]
    enc = LangGraphEventEncoder()
    rt = LangGraphRemoteRuntime(FakeRemoteAgent(events), enc)
    from deepagents_code.domain.events import TurnRequest

    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")
    out = [e async for e in rt.stream_turn(req)]
    assert [e.kind for e in out] == [
        RunEventKind.turn_started,
        RunEventKind.text_delta,
        RunEventKind.tool_started,
        RunEventKind.tool_finished,
        RunEventKind.approval_required,
        RunEventKind.turn_finished,
    ]
    assert [e.sequence for e in out] == [1, 2, 3, 4, 5, 6]


@pytest.mark.asyncio
async def test_remote_runtime_ignores_unrelated_custom_events():
    events = [
        ((), "messages", {"foo": "bar"}),
        _ev(1, "turn_started"),
        _ev(2, "turn_finished"),
    ]
    enc = LangGraphEventEncoder()
    rt = LangGraphRemoteRuntime(FakeRemoteAgent(events), enc)
    from deepagents_code.domain.events import TurnRequest

    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")
    out = [e async for e in rt.stream_turn(req)]
    assert len(out) == 2


@pytest.mark.asyncio
async def test_remote_runtime_rejects_malformed_public_event():
    events = [
        ((), "events", {"session_id": "s1", "turn_id": "t1", "sequence": 1, "kind": "text_delta", "payload": "not-a-dict"}),
    ]
    enc = LangGraphEventEncoder()
    rt = LangGraphRemoteRuntime(FakeRemoteAgent(events), enc)
    from deepagents_code.domain.events import TurnRequest

    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")
    with pytest.raises(RunProtocolError):
        async for _ in rt.stream_turn(req):
            pass


@pytest.mark.asyncio
async def test_remote_runtime_forwards_cancel_to_owned_run():
    events = []
    agent = FakeRemoteAgent(events)
    enc = LangGraphEventEncoder()
    rt = LangGraphRemoteRuntime(agent, enc)
    await rt.cancel_turn("s1", "t1")
    assert len(agent.cancelled) == 1


def test_encoder_returns_none_for_unsupported_mode():
    enc = LangGraphEventEncoder()
    assert enc.encode((), "internal", {"kind": "text_delta"}) is None


def test_encoder_returns_none_for_internal_observation():
    enc = LangGraphEventEncoder()
    assert enc.encode((), "messages", {"foo": "bar"}) is None
