"""Truthful, compact summaries derived from runtime lifecycle events."""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import TYPE_CHECKING, Literal

from niki_agent.domain.events import RunEvent, RunEventKind

if TYPE_CHECKING:
    from collections.abc import Sequence

_FILE_MUTATION_TOOLS = frozenset({"write_file", "edit_file", "delete"})
_TERMINAL_STATUS: dict[RunEventKind, Literal["completed", "failed", "cancelled"]] = {
    RunEventKind.turn_finished: "completed",
    RunEventKind.turn_failed: "failed",
    RunEventKind.turn_cancelled: "cancelled",
}


@dataclass(frozen=True, slots=True)
class TurnSummaryData:
    """Event-derived counts and terminal status for one completed run."""

    changed_file_count: int
    tool_count: int
    status: Literal["completed", "failed", "cancelled"]
    elapsed_seconds: float


class TurnSummary:
    """Build and render summaries without inferring unobserved changes."""

    @staticmethod
    def from_events(
        events: Sequence[RunEvent], elapsed_seconds: float
    ) -> TurnSummaryData:
        """Count completed calls and event-confirmed file mutations.

        File evidence comes from a built-in mutation tool's `tool_started`
        event and its matching successful `tool_finished` event. The runtime
        adapter stores a one-way file ID, so no absolute path is retained here.

        Returns:
            The distinct completed tool count and verified file-change count.

        Raises:
            ValueError: If elapsed time is invalid or no terminal event exists.
        """
        if (
            isinstance(elapsed_seconds, bool)
            or not isinstance(elapsed_seconds, (int, float))
            or not math.isfinite(elapsed_seconds)
            or elapsed_seconds < 0
        ):
            msg = "elapsed time must be a finite non-negative number"
            raise ValueError(msg)

        terminal = [event for event in events if event.kind in _TERMINAL_STATUS]
        if not terminal:
            msg = "events must contain a terminal turn event"
            raise ValueError(msg)
        terminal_event = max(terminal, key=lambda event: event.sequence)

        started_tools: dict[str, tuple[str, str | None]] = {}
        finished_calls: set[str] = set()
        changed_files: set[str] = set()
        for event in events:
            payload = event.payload
            call_id = payload.get("call_id")
            if not isinstance(call_id, str) or not call_id:
                continue
            if event.kind == RunEventKind.tool_started:
                tool_name = payload.get("tool_name")
                if not isinstance(tool_name, str):
                    continue
                file_id = payload.get("file_id")
                started_tools[call_id] = (
                    tool_name,
                    file_id if isinstance(file_id, str) and file_id else None,
                )
            elif event.kind == RunEventKind.tool_finished:
                finished_calls.add(call_id)
                started = started_tools.get(call_id)
                tool_name = payload.get("tool_name")
                status = payload.get("status")
                if (
                    started is not None
                    and started[0] in _FILE_MUTATION_TOOLS
                    and (tool_name is None or tool_name == started[0])
                    and status == "success"
                    and started[1] is not None
                ):
                    changed_files.add(started[1])

        return TurnSummaryData(
            changed_file_count=len(changed_files),
            tool_count=len(finished_calls),
            status=_TERMINAL_STATUS[terminal_event.kind],
            elapsed_seconds=float(elapsed_seconds),
        )

    @staticmethod
    def render(data: TurnSummaryData) -> str:
        """Format one plain-text line suitable for the Niki transcript.

        Returns:
            A concise status, duration, tool count, and verified file count.
        """
        status = data.status.capitalize()
        tools = f"{data.tool_count} tool" + ("" if data.tool_count == 1 else "s")
        files = (
            f"{data.changed_file_count} file change"
            + ("" if data.changed_file_count == 1 else "s")
            + " reported"
        )
        return f"{status} in {data.elapsed_seconds:.1f}s | {tools} | {files}"
