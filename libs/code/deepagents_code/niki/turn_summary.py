"""Turn summary data derived from validated run events."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal, Sequence

from deepagents_code.domain.events import RunEvent, RunEventKind


@dataclass(frozen=True, slots=True)
class TurnSummaryData:
    changed_file_count: int
    tool_count: int
    status: Literal["completed", "failed", "cancelled"]
    elapsed_seconds: float


class TurnSummary:
    @staticmethod
    def from_events(events: Sequence[RunEvent], elapsed_seconds: float) -> TurnSummaryData:
        tool_count = sum(
            1 for e in events if e.kind in (RunEventKind.tool_started, RunEventKind.tool_finished)
        ) // 2 if False else sum(1 for e in events if e.kind == RunEventKind.tool_finished)
        changed_files: set[str] = set()
        for e in events:
            if e.kind == RunEventKind.tool_finished:
                payload = e.payload or {}
                path = payload.get("path") or payload.get("file")
                if isinstance(path, str):
                    changed_files.add(path)
        # only count changed files if there's event evidence (tool_finished with path)
        changed_file_count = len(changed_files)
        status: Literal["completed", "failed", "cancelled"] = "completed"
        for e in events:
            if e.kind == RunEventKind.turn_failed:
                status = "failed"
            elif e.kind == RunEventKind.turn_cancelled:
                status = "cancelled"
            elif e.kind == RunEventKind.turn_finished:
                status = "completed"
        return TurnSummaryData(
            changed_file_count=changed_file_count,
            tool_count=tool_count,
            status=status,
            elapsed_seconds=elapsed_seconds,
        )
