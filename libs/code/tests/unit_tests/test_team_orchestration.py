"""Tests for agent team orchestration."""

from __future__ import annotations

import asyncio
from typing import TYPE_CHECKING, Any, cast

import pytest

from niki_agent.application.orchestration.team import (
    InMemoryTeamChannel,
    InMemoryTeamCheckpointStore,
    TeamCoordinator,
    TeamProtocolError,
)
from niki_agent.domain.team import TeamMessage, TeamPlan, TeamTask, TeamTaskResult

if TYPE_CHECKING:
    from niki_agent.application.ports.agents import AgentTaskPort


class FakeAgentPort:
    def __init__(self, results=None, delay=0.0) -> None:
        self.results = results or {}
        self.delay = delay
        self.calls = []
        self.active = 0
        self.max_active = 0

    async def run(self, task, *, session_id, channel):
        _ = session_id, channel
        self.calls.append(task.task_id)
        self.active += 1
        self.max_active = max(self.active, self.max_active)
        try:
            if self.delay:
                await asyncio.sleep(self.delay)
            status = self.results.get(task.task_id, "completed")
            return TeamTaskResult(task.task_id, status, f"done {task.task_id}")
        finally:
            self.active -= 1


def _coordinator(plan: TeamPlan, port: AgentTaskPort) -> TeamCoordinator:
    return TeamCoordinator(plan, port, checkpoint=InMemoryTeamCheckpointStore())


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
    port = FakeAgentPort(delay=0.01)
    coord = _coordinator(plan, port)
    out = [e async for e in coord.run(session_id="s1")]
    assert len(out) == 4
    assert set(port.calls) == {"a", "b"}
    assert port.max_active == 2
    assert sum(event.payload["status"] == "completed" for event in out) == 2


@pytest.mark.asyncio
async def test_team_respects_max_parallel_limit():
    plan = TeamPlan(
        team_id="team1",
        tasks=tuple(
            TeamTask(task_id=str(i), agent_name="agent", prompt="p") for i in range(5)
        ),
        max_parallel=2,
    )
    port = FakeAgentPort(delay=0.01)
    async for _ in _coordinator(plan, port).run(session_id="s1"):
        pass
    assert port.max_active == 2


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
    coord = _coordinator(plan, port)
    out = [e async for e in coord.run(session_id="s1")]
    assert len(out) == 4
    assert port.calls.index("a") < port.calls.index("b")


@pytest.mark.asyncio
async def test_team_messages_are_ordered_and_isolated_by_team():
    store = InMemoryTeamCheckpointStore()
    channel = InMemoryTeamChannel(store, session_id="s1")
    msg = channel.make_message("team1", "a", "b", {"hi": 1})
    await channel.send(msg)
    second = channel.make_message("team1", "a", "b", {"later": 2})
    await channel.send(second)
    private = channel.make_message("team1", "a", "c", {"private": 3})
    await channel.send(private)
    received = await channel.receive("team1", "b", 0)
    assert [(message.sequence, message.body) for message in received] == [
        (1, {"hi": 1}),
        (2, {"later": 2}),
    ]
    # isolated by team
    received2 = await channel.receive("team2", "b", 0)
    assert len(received2) == 0
    another_session = InMemoryTeamChannel(store, session_id="other")
    assert await another_session.receive("team1", "b", 0) == ()


@pytest.mark.asyncio
async def test_team_messages_survive_resume_checkpoint():
    store = InMemoryTeamCheckpointStore()
    channel = InMemoryTeamChannel(store, session_id="s1")
    msg = channel.make_message("team1", "a", "b", {"hi": 1})
    await channel.send(msg)
    resumed = InMemoryTeamChannel(store, session_id="s1")
    received = await resumed.receive("team1", "b", 0)
    assert [(message.sequence, message.body) for message in received] == [
        (1, {"hi": 1})
    ]
    next_message = resumed.make_message("team1", "a", "b", {"next": True})
    await resumed.send(next_message)
    assert [message.sequence for message in await resumed.receive("team1", "b", 1)] == [
        2
    ]


def test_team_message_rejects_non_json_object_keys():
    with pytest.raises(TeamProtocolError):
        TeamMessage(
            "team", "message", 0, "a", "b", cast("Any", {"nested": {1: "value"}})
        )


def test_team_task_result_rejects_unknown_status():
    with pytest.raises(TeamProtocolError):
        TeamTaskResult("task", cast("Any", "running"), "not terminal")


@pytest.mark.asyncio
@pytest.mark.parametrize(
    ("sender_task_id", "recipient_task_id"),
    [("intruder", "a"), ("a", "intruder")],
)
async def test_team_rejects_messages_from_non_members(
    sender_task_id: str, recipient_task_id: str
) -> None:
    class IntrudingAgent:
        async def run(self, task, *, session_id, channel) -> TeamTaskResult:
            assert session_id == "s1"
            await channel.send(
                TeamMessage(
                    "team1",
                    "intrusion",
                    0,
                    sender_task_id,
                    recipient_task_id,
                    {},
                )
            )
            return TeamTaskResult(task.task_id, "completed", "done")

    plan = TeamPlan("team1", (TeamTask("a", "agent", "p"),), 1)
    events = [
        e async for e in _coordinator(plan, IntrudingAgent()).run(session_id="s1")
    ]
    assert [
        (e.kind.value, e.payload["status"])
        for e in events
        if e.payload["status"] != "running"
    ] == [("task_failed", "failed")]


@pytest.mark.asyncio
async def test_team_channel_detaches_message_bodies_from_workers():
    class MutatingAgent:
        async def run(self, task, *, session_id, channel) -> TeamTaskResult:
            _ = session_id, channel
            message = TeamMessage(
                "team", "message", 0, task.task_id, task.task_id, {"v": 1}
            )
            await channel.send(message)
            message.body["v"] = 2
            received = await channel.receive("team", task.task_id, 0)
            received[0].body["v"] = 3
            reread = await channel.receive("team", task.task_id, 0)
            return TeamTaskResult(task.task_id, "completed", str(reread[0].body["v"]))

    plan = TeamPlan("team", (TeamTask("task", "agent", "p"),), 1)
    events = [e async for e in _coordinator(plan, MutatingAgent()).run(session_id="s1")]
    assert events[-1].payload["summary"] == "1"


def test_team_rejects_duplicate_ids_and_cycles():
    with pytest.raises(TeamProtocolError):
        TeamPlan(
            team_id="t",
            tasks=(TeamTask("a", "ag", "p"), TeamTask("a", "ag", "p")),
            max_parallel=1,
        )
    with pytest.raises(TeamProtocolError):
        TeamPlan(
            team_id="t",
            tasks=(
                TeamTask("a", "ag", "p", depends_on=("b",)),
                TeamTask("b", "ag", "p", depends_on=("a",)),
            ),
            max_parallel=1,
        )


@pytest.mark.parametrize("max_parallel", [0, -1, True, 1.5])
def test_team_rejects_invalid_max_parallel(max_parallel):
    with pytest.raises(TeamProtocolError):
        TeamPlan("team", (TeamTask("a", "agent", "p"),), max_parallel)


@pytest.mark.asyncio
async def test_failed_team_member_is_reported_and_siblings_are_cancelled():
    plan = TeamPlan(
        team_id="team1",
        tasks=(
            TeamTask(task_id="a", agent_name="agent", prompt="p"),
            TeamTask(task_id="b", agent_name="agent", prompt="p"),
        ),
        max_parallel=2,
    )

    class FailingSiblingPort:
        def __init__(self) -> None:
            self.sibling_started = asyncio.Event()
            self.sibling_cancelled = False

        async def run(self, task, *, session_id, channel) -> TeamTaskResult:
            assert session_id == "s1"
            assert channel is not None
            if task.task_id == "a":
                await self.sibling_started.wait()
                return TeamTaskResult("a", "failed", "agent failed")
            self.sibling_started.set()
            try:
                await asyncio.Future()
            except asyncio.CancelledError:
                self.sibling_cancelled = True
                raise
            msg = "waiting sibling returned unexpectedly"
            raise AssertionError(msg)

    port = FailingSiblingPort()
    coord = _coordinator(plan, port)
    out = [e async for e in coord.run(session_id="s1")]
    statuses = [e.payload["status"] for e in out]
    assert "failed" in statuses
    assert "cancelled" in statuses
    assert port.sibling_cancelled


@pytest.mark.asyncio
async def test_failed_dependency_skips_downstream_task():
    plan = TeamPlan(
        "team1",
        (
            TeamTask("parent", "agent", "p"),
            TeamTask("child", "agent", "p", depends_on=("parent",)),
        ),
        1,
    )
    events = [
        event
        async for event in _coordinator(
            plan, FakeAgentPort(results={"parent": "failed"})
        ).run(session_id="s1")
    ]
    assert any(
        event.payload["task_id"] == "child" and event.payload["status"] == "skipped"
        for event in events
    )


@pytest.mark.asyncio
async def test_closing_event_stream_cancels_and_checkpoints_active_worker():
    class WaitingAgent:
        def __init__(self) -> None:
            self.started = asyncio.Event()
            self.cancelled = False

        async def run(self, task, *, session_id, channel) -> TeamTaskResult:
            _ = task, session_id, channel
            self.started.set()
            try:
                await asyncio.Future()
            except asyncio.CancelledError:
                self.cancelled = True
                raise
            msg = "waiting task returned unexpectedly"
            raise AssertionError(msg)

    plan = TeamPlan("team", (TeamTask("task", "agent", "p"),), 1)
    port = WaitingAgent()
    store = InMemoryTeamCheckpointStore()
    events = TeamCoordinator(plan, port, checkpoint=store).run(session_id="session")
    assert (await anext(events)).payload["status"] == "running"
    await port.started.wait()

    await cast("Any", events).aclose()

    checkpoint = await store.load("session", "team")
    assert port.cancelled
    assert checkpoint is not None
    assert [(result.task_id, result.status) for result in checkpoint.results] == [
        ("task", "cancelled")
    ]


@pytest.mark.asyncio
async def test_completed_team_tasks_are_restored_from_checkpoint():
    store = InMemoryTeamCheckpointStore()
    plan = TeamPlan("team1", (TeamTask("a", "agent", "p"),), 1)
    first = FakeAgentPort()
    async for _ in TeamCoordinator(plan, first, checkpoint=store).run(session_id="s1"):
        pass

    resumed = FakeAgentPort()
    events = [
        e
        async for e in TeamCoordinator(plan, resumed, checkpoint=store).run(
            session_id="s1"
        )
    ]
    assert resumed.calls == []
    assert [event.payload["status"] for event in events] == ["completed"]
    assert events[0].sequence == 3


@pytest.mark.asyncio
async def test_team_resume_rejects_a_changed_task_list():
    store = InMemoryTeamCheckpointStore()
    initial = TeamPlan("team", (TeamTask("first", "agent", "p"),), 1)
    async for _ in TeamCoordinator(initial, FakeAgentPort(), checkpoint=store).run(
        session_id="s1"
    ):
        pass

    changed = TeamPlan("team", (TeamTask("replacement", "agent", "p"),), 1)
    with pytest.raises(TeamProtocolError, match="task list changed"):
        async for _ in TeamCoordinator(changed, FakeAgentPort(), checkpoint=store).run(
            session_id="s1"
        ):
            pass
