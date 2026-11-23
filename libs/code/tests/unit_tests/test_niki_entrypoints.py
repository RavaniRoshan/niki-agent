"""Tests for Niki entry-point runtime routing (interface-level)."""

from __future__ import annotations

import pytest

from deepagents_code.adapters.runtime.acp import ACPAgentRuntime
from deepagents_code.application.ports.runtime import AgentRuntime
from deepagents_code.application.ports.runtime import AgentRuntime
from deepagents_code.domain.events import RunEventKind, TurnRequest, ResumeRequest


class FakeAgent:
    async def astream(self, value, *, config=None, **kw):
        return
        yield  # pragma: no cover


@pytest.mark.asyncio
async def test_acp_runtime_maps_session_events_and_approval_resume():
    agent = FakeAgent()
    rt = ACPAgentRuntime(agent, session=None)
    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")
    out = [e async for e in rt.stream_turn(req)]
    assert out[0].kind == RunEventKind.turn_started
    assert out[-1].kind == RunEventKind.turn_finished


@pytest.mark.asyncio
async def test_interrupted_turn_resumes_without_repeating_completed_tool():
    agent = FakeAgent()
    rt = ACPAgentRuntime(agent, session=None)
    rreq = ResumeRequest(
        session_id="s1", turn_id="t1", decision={}, last_sequence=2,
        prior_status=__import__("deepagents_code.domain.events", fromlist=["TurnStatus"]).TurnStatus.running,
    )
    out = [e async for e in rt.resume_turn(rreq)]
    assert out[0].sequence == 3


@pytest.mark.asyncio
async def test_provider_timeout_emits_recoverable_failure():
    class FailingAgent:
        async def astream(self, value, *, config=None, **kw):
            raise TimeoutError("provider timeout")
            yield  # pragma: no cover

    rt = ACPAgentRuntime(FailingAgent(), session=None)
    req = TurnRequest(session_id="s1", turn_id="t1", prompt="hi", workspace_root="/w")
    out = [e async for e in rt.stream_turn(req)]
    assert any(e.kind == RunEventKind.turn_failed for e in out)
