"""Agent team coordinator."""

from __future__ import annotations

import asyncio
from collections.abc import AsyncIterator
from typing import Any

from deepagents_code.application.ports.agents import AgentTaskPort
from deepagents_code.domain.events import RunEvent, RunEventKind
from deepagents_code.domain.team import (
    TeamChannel,
    TeamMessage,
    TeamPlan,
    TeamTask,
    TeamTaskResult,
    TeamTaskStatus,
)


class TeamProtocolError(ValueError):
    pass


class InMemoryTeamChannel:
    def __init__(self) -> None:
        self._messages: list[TeamMessage] = []
        self._lock = asyncio.Lock()
        self._counter = 0

    async def send(self, message: TeamMessage) -> None:
        async with self._lock:
            self._messages.append(message)

    async def receive(
        self, team_id: str, recipient_task_id: str, after_sequence: int
    ) -> list[TeamMessage]:
        async with self._lock:
            return [
                m
                for m in self._messages
                if m.team_id == team_id
                and m.recipient_task_id == recipient_task_id
                and m.sequence > after_sequence
            ]

    def next_sequence(self) -> int:
        self._counter += 1
        return self._counter

    def make_message(
        self,
        team_id: str,
        sender_task_id: str,
        recipient_task_id: str,
        body: dict,
    ) -> TeamMessage:
        return TeamMessage(
            team_id=team_id,
            message_id=f"m{self.next_sequence()}",
            sequence=self._counter,
            sender_task_id=sender_task_id,
            recipient_task_id=recipient_task_id,
            body=body,
        )


def _validate_plan(plan: TeamPlan) -> None:
    if plan.max_parallel < 1:
        raise TeamProtocolError("max_parallel must be >= 1")
    ids = [t.task_id for t in plan.tasks]
    if len(set(ids)) != len(ids):
        raise TeamProtocolError("duplicate task ids")
    id_set = set(ids)
    adj = {t.task_id: list(t.depends_on) for t in plan.tasks}
    for tid, deps in adj.items():
        for d in deps:
            if d not in id_set:
                raise TeamProtocolError(f"unknown dependency {d}")
    # cycle detection
    WHITE, GRAY, BLACK = 0, 1, 2
    color = {tid: WHITE for tid in id_set}

    def dfs(node):
        color[node] = GRAY
        for dep in adj.get(node, []):
            if color[dep] == GRAY:
                raise TeamProtocolError("cycle detected")
            if color[dep] == WHITE:
                dfs(dep)
        color[node] = BLACK

    for tid in id_set:
        if color[tid] == WHITE:
            dfs(tid)

    # validate messages from non-members rejected handled in channel usage


class TeamCoordinator:
    def __init__(
        self,
        plan: TeamPlan,
        agent_port: AgentTaskPort,
        channel: TeamChannel | None = None,
    ) -> None:
        _validate_plan(plan)
        self.plan = plan
        self.agent_port = agent_port
        self.channel = channel if channel is not None else InMemoryTeamChannel()

    async def run(self, *, session_id: str) -> AsyncIterator[RunEvent]:
        plan = self.plan
        task_map = {t.task_id: t for t in plan.tasks}
        dependents: dict[str, list[str]] = {t.task_id: [] for t in plan.tasks}
        for t in plan.tasks:
            for d in t.depends_on:
                dependents[d].append(t.task_id)

        status: dict[str, TeamTaskStatus] = {}
        results: dict[str, TeamTaskResult] = {}
        remaining_deps: dict[str, int] = {t.task_id: len(t.depends_on) for t in plan.tasks}

        event_queue: asyncio.Queue = asyncio.Queue()
        seq_counter = [0]

        def emit(task_id, st, summary=""):
            seq_counter[0] += 1
            event_queue.put_nowait((task_id, st, summary, seq_counter[0]))

        async def run_task(task_id: str):
            task = task_map[task_id]
            # check deps
            for d in task.depends_on:
                if status.get(d) != "completed":
                    status[task_id] = "skipped"
                    results[task_id] = TeamTaskResult(task_id, "skipped", f"dependency {d} not completed")
                    emit(task_id, "skipped", f"dependency {d} not completed")
                    # unblock
                    for dep in dependents[task_id]:
                        remaining_deps[dep] = max(0, remaining_deps[dep] - 1)
                    return
            try:
                res = await self.agent_port.run(task, session_id=session_id, channel=self.channel)
            except Exception as exc:
                res = TeamTaskResult(task_id, "failed", str(exc))
            status[task_id] = res.status
            results[task_id] = res
            emit(task_id, res.status, res.summary)
            for dep in dependents[task_id]:
                remaining_deps[dep] = max(0, remaining_deps[dep] - 1)

        sem = asyncio.Semaphore(plan.max_parallel)
        scheduled: set[str] = set()
        in_flight: set[asyncio.Task] = set()

        def schedule_ready():
            changed = True
            while changed:
                changed = False
                for t in plan.tasks:
                    tid = t.task_id
                    if tid in scheduled or status.get(tid) is not None:
                        continue
                    if remaining_deps.get(tid, 0) <= 0:
                        # check if deps all completed
                        if all(status.get(d) == "completed" for d in t.depends_on) or not t.depends_on:
                            scheduled.add(tid)
                            async def _run(x=tid):
                                async with sem:
                                    await run_task(x)
                            in_flight.add(asyncio.ensure_future(_run()))
                            changed = True
                        elif any(status.get(d) in ("failed", "cancelled", "skipped") for d in t.depends_on):
                            status[tid] = "skipped"
                            results[tid] = TeamTaskResult(tid, "skipped", "dependency failed")
                            emit(tid, "skipped", "dependency failed")
                            scheduled.add(tid)
                            changed = True

        schedule_ready()
        total = len(plan.tasks)
        emitted = 0

        while emitted < total:
            # wait for a completion
            if not in_flight and event_queue.empty():
                # schedule any remaining
                schedule_ready()
                if not in_flight and event_queue.empty():
                    break
            # wait for event or completion
            wait_tasks = list(in_flight)
            if wait_tasks:
                done, _ = await asyncio.wait(wait_tasks, timeout=0.05, return_when=asyncio.FIRST_COMPLETED)
                for t in done:
                    in_flight.discard(t)
                    try:
                        t.result()
                    except Exception:
                        pass
                schedule_ready()
            # drain events
            while not event_queue.empty():
                tid, st, summary, seq = event_queue.get_nowait()
                emitted += 1
                kind_map = {
                    "completed": RunEventKind.task_finished,
                    "failed": RunEventKind.task_failed,
                    "cancelled": RunEventKind.task_failed,
                    "skipped": RunEventKind.task_skipped,
                }
                yield RunEvent(
                    schema_version=1,
                    event_id=f"{tid}-{st}",
                    session_id=session_id,
                    turn_id=plan.team_id,
                    sequence=seq,
                    kind=kind_map.get(st, RunEventKind.task_finished),
                    payload={"task_id": tid, "status": st, "summary": summary},
                )
                if emitted >= total:
                    break

        # cancel any remaining
        for t in list(in_flight):
            t.cancel()
