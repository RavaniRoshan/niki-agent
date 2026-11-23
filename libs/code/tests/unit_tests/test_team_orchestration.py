"""Tests for agent team orchestration."""

from __future__ import annotations

import pytest

from deepagents_code.application.orchestration.team import (
    InMemoryTeamChannel,
    TeamCoordinator,
    TeamProtocolError,
)
from deepagents_code.application.ports.agents import AgentTaskPort
from deepagents_code.domain.team import TeamPlan, TeamTask, TeamTaskResult


class FakeAgentPort:
    def __init__(self, results=None, delay=0.0):
        self.results = results or {}
        self.delay = delay
        self.calls = []

    async def run(self, task, *, session_id, channel):
        self.calls.append(task.task_id)
        if self.delay:
            import asyncio
            await asyncio.sleep(self.delay)
        status = self.results.get(task.task_id, "completed")
        return TeamTaskResult(task.task_id, status, f"done {task.task_id}")


@pytest.mark.asyncio
async def test_team_runs_independent_tasks_concurrently():
    plan = TeamPlan(
        team_id="team1",
        tasks=(
            TeamTask(task_id="a", agent_name="agent", prompt="p"),
            TeamTask(task_id="b", agent_name="agent", prompt="p"),
        ),
        max_parallel=2,
    )
    port = FakeAgentPort()
    coord = TeamCoordinator(plan, port)
    out = [e async for e in coord.run(session_id="s1")]
    assert len(out) == 2
    assert set(port.calls) == {"a", "b"}


@pytest.mark.asyncio
async def test_team_waits_for_dependencies():
    plan = TeamPlan(
        team_id="team1",
        tasks=(
            TeamTask(task_id="a", agent_name="agent", prompt="p"),
            TeamTask(task_id="b", agent_name="agent", prompt="p", depends_on=("a",)),
        ),
        max_parallel=2,
    )
    port = FakeAgentPort()
    coord = TeamCoordinator(plan, port)
    out = [e async for e in coord.run(session_id="s1")]
    assert len(out) == 2
    assert port.calls.index("a") < port.calls.index("b")


@pytest.mark.asyncio
async def test_team_messages_are_ordered_and_isolated_by_team():
    channel = InMemoryTeamChannel()
    msg = channel.make_message("team1", "a", "b", {"hi": 1})
    await channel.send(msg)
    received = await channel.receive("team1", "b", 0)
    assert len(received) == 1
    # isolated by team
    received2 = await channel.receive("team2", "b", 0)
    assert len(received2) == 0


@pytest.mark.asyncio
async def test_team_messages_survive_resume_checkpoint():
    channel = InMemoryTeamChannel()
    msg = channel.make_message("team1", "a", "b", {"hi": 1})
    await channel.send(msg)
    # receive again after sequence
    received = await channel.receive("team1", "b", msg.sequence)
    assert len(received) == 0
    received2 = await channel.receive("team1", "b", 0)
    assert len(received2) == 1


@pytest.mark.asyncio
async def test_team_rejects_duplicate_ids_and_cycles():
    with pytest.raises(TeamProtocolError):
        TeamPlan(team_id="t", tasks=(TeamTask("a", "ag", "p"), TeamTask("a", "ag", "p")), max_parallel=1)
    with pytest.raises(TeamProtocolError):
        TeamPlan(team_id="t", tasks=(TeamTask("a", "ag", "p", depends_on=("b",)), TeamTask("b", "ag", "p", depends_on=("a",))), max_parallel=1)
    with pytest.raises(TeamProtocolError):
        TeamPlan(team_id="t", tasks=(TeamTask("a", "ag", "p"),), max_parallel=0)


@pytest.mark.asyncio
async def test_failed_team_member_is_reported_and_siblings_are_cancelled():
    plan = TeamPlan(
        team_id="team1",
        tasks=(
            TeamTask(task_id="a", agent_name="agent", prompt="p"),
            TeamTask(task_id="b", agent_name="agent", prompt="p", depends_on=("a",)),
        ),
        max_parallel=2,
    )
    port = FakeAgentPort(results={"a": "failed"})
    coord = TeamCoordinator(plan, port)
    out = [e async for e in coord.run(session_id="s1")]
    statuses = [e.payload["status"] for e in out]
    assert "failed" in statuses
    assert "skipped" in statuses
