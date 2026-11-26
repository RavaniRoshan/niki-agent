"""Checkpoint-backed agent team coordination."""

from __future__ import annotations

import asyncio
from copy import deepcopy
from dataclasses import replace
from typing import TYPE_CHECKING, NoReturn
from uuid import uuid4

from niki_agent.domain.events import RunEvent, RunEventKind
from niki_agent.domain.team import (
    TeamCheckpoint,
    TeamMessage,
    TeamPlan,
    TeamProtocolError as TeamProtocolError,
    TeamTaskResult,
    validate_team_plan,
)

if TYPE_CHECKING:
    from collections.abc import AsyncIterator, Sequence

    from niki_agent.application.ports.agents import (
        AgentTaskPort,
        TeamCheckpointPort,
    )
    from niki_agent.domain.events import JsonObject
    from niki_agent.domain.team import TeamChannel, TeamTask

_TERMINAL_STATUSES = frozenset({"completed", "failed", "cancelled", "skipped"})
_FAILED_STATUSES = frozenset({"failed", "cancelled", "skipped"})
_EVENT_KIND_BY_STATUS = {
    "completed": RunEventKind.task_finished,
    "failed": RunEventKind.task_failed,
    "cancelled": RunEventKind.task_failed,
    "skipped": RunEventKind.task_skipped,
}


def _raise_protocol_error(msg: str) -> NoReturn:
    """Raise an already-formatted team protocol failure.

    Raises:
        TeamProtocolError: The supplied team contract error.
    """
    raise TeamProtocolError(msg)


class InMemoryTeamCheckpointStore:
    """Test/local checkpoint port; production supplies the session checkpointer."""

    def __init__(self) -> None:
        """Create an isolated in-memory checkpoint store."""
        self._states: dict[tuple[str, str], TeamCheckpoint] = {}
        self._lock = asyncio.Lock()

    async def load(self, session_id: str, team_id: str) -> TeamCheckpoint | None:
        """Read a detached snapshot for the session-scoped team.

        Returns:
            The latest checkpoint or `None` if the team has not started.
        """
        async with self._lock:
            checkpoint = self._states.get((session_id, team_id))
            return deepcopy(checkpoint)

    async def save(self, session_id: str, checkpoint: TeamCheckpoint) -> None:
        """Replace the latest snapshot for the session-scoped team."""
        async with self._lock:
            self._states[session_id, checkpoint.team_id] = deepcopy(checkpoint)


class InMemoryTeamChannel:
    """Small checkpoint-backed channel for tests and local coordinator use."""

    def __init__(
        self,
        checkpoint: TeamCheckpointPort | None = None,
        *,
        session_id: str = "local",
    ) -> None:
        """Create a test channel backed by the supplied checkpoint port."""
        self._checkpoint = checkpoint or InMemoryTeamCheckpointStore()
        self._session_id = session_id
        self._lock = asyncio.Lock()

    @staticmethod
    def make_message(
        team_id: str,
        sender_task_id: str,
        recipient_task_id: str,
        body: JsonObject,
    ) -> TeamMessage:
        """Create an unsequenced message.

        Returns:
            A message with sequence zero for checkpoint-side numbering.
        """
        return TeamMessage(
            team_id=team_id,
            message_id=uuid4().hex,
            sequence=0,
            sender_task_id=sender_task_id,
            recipient_task_id=recipient_task_id,
            body=body,
        )

    async def send(self, message: TeamMessage) -> None:
        """Append a message and assign its next checkpoint sequence."""
        if message.sequence < 0:
            _raise_protocol_error("message sequence cannot be negative")
        async with self._lock:
            checkpoint = await self._load_or_create(message.team_id)
            await self._append_message(checkpoint, message)

    async def receive(
        self, team_id: str, recipient_task_id: str, after_sequence: int
    ) -> Sequence[TeamMessage]:
        """Read messages for one recipient after a saved sequence cursor.

        Returns:
            Messages in checkpoint sequence order.
        """
        if after_sequence < 0:
            _raise_protocol_error("after_sequence cannot be negative")
        checkpoint = await self._checkpoint.load(self._session_id, team_id)
        if checkpoint is None:
            return ()
        return tuple(
            sorted(
                (
                    message
                    for message in checkpoint.messages
                    if message.recipient_task_id == recipient_task_id
                    and message.sequence > after_sequence
                ),
                key=lambda message: message.sequence,
            )
        )

    async def _load_or_create(self, team_id: str) -> TeamCheckpoint:
        checkpoint = await self._checkpoint.load(self._session_id, team_id)
        if checkpoint is not None:
            return checkpoint
        checkpoint = TeamCheckpoint(team_id=team_id, task_ids=())
        await self._checkpoint.save(self._session_id, checkpoint)
        return checkpoint

    async def _append_message(
        self, checkpoint: TeamCheckpoint, message: TeamMessage
    ) -> None:
        existing = next(
            (
                item
                for item in checkpoint.messages
                if item.message_id == message.message_id
            ),
            None,
        )
        if existing is not None:
            if existing == replace(message, sequence=existing.sequence):
                return
            _raise_protocol_error("duplicate message_id with different contents")

        sequence = checkpoint.next_message_sequence + 1
        stored = replace(message, sequence=sequence, body=deepcopy(message.body))
        updated = replace(
            checkpoint,
            messages=(*checkpoint.messages, stored),
            next_message_sequence=sequence,
        )
        await self._checkpoint.save(self._session_id, updated)


class _TeamRuntimeState:
    def __init__(
        self,
        session_id: str,
        checkpoint_port: TeamCheckpointPort,
        checkpoint: TeamCheckpoint,
    ) -> None:
        """Hold one run's serialized state updates and checkpoint writer."""
        self.session_id = session_id
        self.checkpoint_port = checkpoint_port
        self.checkpoint = checkpoint
        self.lock = asyncio.Lock()

    async def record_event(
        self,
        plan: TeamPlan,
        task: TeamTask,
        task_order: int,
        *,
        status: str,
        summary: str = "",
        result: TeamTaskResult | None = None,
    ) -> RunEvent:
        async with self.lock:
            results = {item.task_id: item for item in self.checkpoint.results}
            if result is not None:
                results[result.task_id] = result
            ordered_results = tuple(
                results[task_id]
                for task_id in self.checkpoint.task_ids
                if task_id in results
            )
            sequence = self.checkpoint.event_sequence + 1
            updated = replace(
                self.checkpoint,
                results=ordered_results,
                event_sequence=sequence,
            )
            await self.checkpoint_port.save(self.session_id, updated)
            self.checkpoint = updated

        kind = (
            RunEventKind.task_started
            if status == "running"
            else _EVENT_KIND_BY_STATUS[status]
        )
        return RunEvent(
            schema_version=1,
            event_id=f"{plan.team_id}:{task.task_id}:{status}:{sequence}",
            session_id=self.session_id,
            turn_id=plan.team_id,
            sequence=sequence,
            kind=kind,
            payload={
                "task_id": task.task_id,
                "task_order": task_order,
                "agent_name": task.agent_name,
                "status": status,
                "summary": summary,
            },
        )

    async def append_message(self, message: TeamMessage) -> None:
        async with self.lock:
            existing = next(
                (
                    item
                    for item in self.checkpoint.messages
                    if item.message_id == message.message_id
                ),
                None,
            )
            if existing is not None:
                if existing == replace(message, sequence=existing.sequence):
                    return
                _raise_protocol_error("duplicate message_id with different contents")
            sequence = self.checkpoint.next_message_sequence + 1
            stored = replace(message, sequence=sequence, body=deepcopy(message.body))
            updated = replace(
                self.checkpoint,
                messages=(*self.checkpoint.messages, stored),
                next_message_sequence=sequence,
            )
            await self.checkpoint_port.save(self.session_id, updated)
            self.checkpoint = updated

    async def receive_messages(
        self, recipient_task_id: str, after_sequence: int
    ) -> Sequence[TeamMessage]:
        async with self.lock:
            return deepcopy(
                tuple(
                    sorted(
                        (
                            message
                            for message in self.checkpoint.messages
                            if message.recipient_task_id == recipient_task_id
                            and message.sequence > after_sequence
                        ),
                        key=lambda message: message.sequence,
                    )
                )
            )


class _TaskTeamChannel:
    def __init__(
        self,
        team_id: str,
        task_id: str,
        member_ids: frozenset[str],
        state: _TeamRuntimeState,
    ) -> None:
        """Bind message access to one current team task."""
        self._team_id = team_id
        self._task_id = task_id
        self._member_ids = member_ids
        self._state = state

    async def send(self, message: TeamMessage) -> None:
        if message.team_id != self._team_id:
            _raise_protocol_error("message belongs to a different team")
        if message.sender_task_id != self._task_id:
            _raise_protocol_error("sender must be the current team member")
        if message.sender_task_id not in self._member_ids:
            _raise_protocol_error("sender is not a team member")
        if message.recipient_task_id not in self._member_ids:
            _raise_protocol_error("recipient is not a team member")
        if message.sequence < 0:
            _raise_protocol_error("message sequence cannot be negative")
        await self._state.append_message(message)

    async def receive(
        self, team_id: str, recipient_task_id: str, after_sequence: int
    ) -> Sequence[TeamMessage]:
        if team_id != self._team_id:
            _raise_protocol_error("cannot read another team's mailbox")
        if recipient_task_id != self._task_id:
            _raise_protocol_error("a team member can read only its own mailbox")
        if recipient_task_id not in self._member_ids:
            _raise_protocol_error("recipient is not a team member")
        if after_sequence < 0:
            _raise_protocol_error("after_sequence cannot be negative")
        return await self._state.receive_messages(recipient_task_id, after_sequence)


class TeamCoordinator:
    """Run a validated task DAG with bounded concurrency and checkpointed state."""

    def __init__(
        self,
        plan: TeamPlan,
        agent_port: AgentTaskPort,
        *,
        checkpoint: TeamCheckpointPort,
    ) -> None:
        """Bind a task DAG to its worker port and session checkpoint."""
        validate_team_plan(plan)
        self.plan = plan
        self.agent_port = agent_port
        self.checkpoint = checkpoint

    async def run(self, *, session_id: str) -> AsyncIterator[RunEvent]:
        """Run pending tasks and emit coordinator lifecycle events.

        Closing this iterator cancels active workers and checkpoints their outcomes.

        Args:
            session_id: Owning server session used to load and save team state.

        Yields:
            `RunEvent` objects for task starts and terminal outcomes.
        """
        state = await self._load_state(session_id)
        task_order = {task.task_id: index for index, task in enumerate(self.plan.tasks)}
        tasks = {task.task_id: task for task in self.plan.tasks}
        results = {result.task_id: result for result in state.checkpoint.results}
        members = frozenset(tasks)
        pending = {task_id for task_id in tasks if task_id not in results}
        active: dict[str, asyncio.Task[TeamTaskResult]] = {}

        for task in self.plan.tasks:
            if result := results.get(task.task_id):
                yield await state.record_event(
                    self.plan,
                    task,
                    task_order[task.task_id],
                    status=result.status,
                    summary=result.summary,
                )

        if any(result.status in _FAILED_STATUSES for result in results.values()):
            for event in await self._settle_pending(
                pending, results, state, task_order
            ):
                yield event
            return

        try:
            while pending or active:
                started_any = False
                for task in self.plan.tasks:
                    task_id = task.task_id
                    if task_id not in pending or len(active) >= self.plan.max_parallel:
                        continue
                    dependency_statuses = [
                        results[d].status for d in task.depends_on if d in results
                    ]
                    if any(
                        status in _FAILED_STATUSES for status in dependency_statuses
                    ):
                        pending.remove(task_id)
                        result = TeamTaskResult(task_id, "skipped", "dependency failed")
                        results[task_id] = result
                        yield await state.record_event(
                            self.plan,
                            task,
                            task_order[task_id],
                            status=result.status,
                            summary=result.summary,
                            result=result,
                        )
                        started_any = True
                        continue
                    if len(dependency_statuses) != len(task.depends_on):
                        continue
                    pending.remove(task_id)
                    channel = _TaskTeamChannel(
                        self.plan.team_id, task_id, members, state
                    )
                    event = await state.record_event(
                        self.plan,
                        task,
                        task_order[task_id],
                        status="running",
                    )
                    active[task_id] = asyncio.create_task(
                        self._execute(task, session_id=session_id, channel=channel)
                    )
                    yield event
                    started_any = True

                if not active:
                    if pending and not started_any:
                        # A validated DAG should always have a ready task. Treat a
                        # checkpoint mismatch as a visible failure rather than hang.
                        for event in await self._settle_pending(
                            pending, results, state, task_order
                        ):
                            yield event
                    continue

                done, _ = await asyncio.wait(
                    active.values(), return_when=asyncio.FIRST_COMPLETED
                )
                completed_ids = sorted(
                    (task_id for task_id, running in active.items() if running in done),
                    key=task_order.__getitem__,
                )
                completed_results = []
                for task_id in completed_ids:
                    running = active.pop(task_id)
                    completed_results.append((task_id, running.result()))

                emitted = []
                failed = False
                for task_id, result in completed_results:
                    results[task_id] = result
                    emitted.append(
                        await state.record_event(
                            self.plan,
                            tasks[task_id],
                            task_order[task_id],
                            status=result.status,
                            summary=result.summary,
                            result=result,
                        )
                    )
                    failed = failed or result.status in _FAILED_STATUSES

                if failed:
                    cancelled_results = await self._cancel_active(active)
                    active.clear()
                    for task_id, result in cancelled_results:
                        results[task_id] = result
                        emitted.append(
                            await state.record_event(
                                self.plan,
                                tasks[task_id],
                                task_order[task_id],
                                status=result.status,
                                summary=result.summary,
                                result=result,
                            )
                        )
                    emitted.extend(
                        await self._settle_pending(pending, results, state, task_order)
                    )
                    for event in emitted:
                        yield event
                    return

                for event in emitted:
                    yield event
        finally:
            cancelled_results = await self._cancel_active(active)
            active.clear()
            for task_id, result in cancelled_results:
                await state.record_event(
                    self.plan,
                    tasks[task_id],
                    task_order[task_id],
                    status=result.status,
                    summary=result.summary,
                    result=result,
                )

    async def _load_state(self, session_id: str) -> _TeamRuntimeState:
        checkpoint = await self.checkpoint.load(session_id, self.plan.team_id)
        task_ids = tuple(task.task_id for task in self.plan.tasks)
        if checkpoint is None:
            checkpoint = TeamCheckpoint(self.plan.team_id, task_ids)
            await self.checkpoint.save(session_id, checkpoint)
        elif checkpoint.team_id != self.plan.team_id:
            _raise_protocol_error("checkpoint belongs to a different team")
        elif not checkpoint.task_ids:
            checkpoint = replace(checkpoint, task_ids=task_ids)
            await self.checkpoint.save(session_id, checkpoint)
        elif checkpoint.task_ids != task_ids:
            _raise_protocol_error("team task list changed since checkpoint")
        self._validate_checkpoint(checkpoint, frozenset(task_ids))
        return _TeamRuntimeState(session_id, self.checkpoint, checkpoint)

    @staticmethod
    def _validate_checkpoint(
        checkpoint: TeamCheckpoint, member_ids: frozenset[str]
    ) -> None:
        result_ids: set[str] = set()
        for result in checkpoint.results:
            if result.task_id not in member_ids or result.task_id in result_ids:
                _raise_protocol_error("checkpoint has an invalid task result")
            if result.status not in _TERMINAL_STATUSES:
                _raise_protocol_error("checkpoint has an invalid task status")
            result_ids.add(result.task_id)
        sequences: set[int] = set()
        message_ids: set[str] = set()
        for message in checkpoint.messages:
            if (
                message.team_id != checkpoint.team_id
                or message.sender_task_id not in member_ids
                or message.recipient_task_id not in member_ids
                or message.sequence < 1
                or message.sequence in sequences
                or message.message_id in message_ids
            ):
                _raise_protocol_error("checkpoint has an invalid team message")
            sequences.add(message.sequence)
            message_ids.add(message.message_id)
        if sequences and max(sequences) > checkpoint.next_message_sequence:
            _raise_protocol_error("checkpoint message sequence is inconsistent")

    async def _execute(
        self, task: TeamTask, *, session_id: str, channel: TeamChannel
    ) -> TeamTaskResult:
        try:
            result = await self.agent_port.run(
                task, session_id=session_id, channel=channel
            )
        except asyncio.CancelledError:
            return TeamTaskResult(
                task.task_id, "cancelled", "cancelled after sibling failure"
            )
        except Exception as exc:  # noqa: BLE001 — worker errors become failed task results.
            return TeamTaskResult(task.task_id, "failed", str(exc))
        if result.task_id != task.task_id or result.status not in _TERMINAL_STATUSES:
            return TeamTaskResult(
                task.task_id, "failed", "agent returned an invalid task result"
            )
        return result

    @staticmethod
    async def _cancel_active(
        active: dict[str, asyncio.Task[TeamTaskResult]],
    ) -> list[tuple[str, TeamTaskResult]]:
        for task in active.values():
            task.cancel()
        results = await asyncio.gather(*active.values(), return_exceptions=True)
        cancelled = []
        for task_id, result in zip(active, results, strict=True):
            if isinstance(result, TeamTaskResult):
                cancelled.append((task_id, result))
            else:
                cancelled.append(
                    (task_id, TeamTaskResult(task_id, "cancelled", "cancelled"))
                )
        return cancelled

    async def _settle_pending(
        self,
        pending: set[str],
        results: dict[str, TeamTaskResult],
        state: _TeamRuntimeState,
        task_order: dict[str, int],
    ) -> list[RunEvent]:
        events = []
        failed_ids = {
            task_id
            for task_id, result in results.items()
            if result.status in _FAILED_STATUSES
        }
        for task in self.plan.tasks:
            if task.task_id not in pending:
                continue
            if self._has_failed_ancestor(task.task_id, failed_ids):
                result = TeamTaskResult(task.task_id, "skipped", "dependency failed")
            else:
                result = TeamTaskResult(
                    task.task_id, "cancelled", "cancelled after sibling failure"
                )
            pending.remove(task.task_id)
            results[task.task_id] = result
            events.append(
                await state.record_event(
                    self.plan,
                    task,
                    task_order[task.task_id],
                    status=result.status,
                    summary=result.summary,
                    result=result,
                )
            )
        return events

    def _has_failed_ancestor(self, task_id: str, failed_ids: set[str]) -> bool:
        task_map = {task.task_id: task for task in self.plan.tasks}
        pending = list(task_map[task_id].depends_on)
        visited: set[str] = set()
        while pending:
            dependency = pending.pop()
            if dependency in failed_ids:
                return True
            if dependency not in visited:
                visited.add(dependency)
                pending.extend(task_map[dependency].depends_on)
        return False
