"""Tests for event-backed terminal turn summaries."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, cast

import pytest

from niki_agent.domain.events import RunEvent, RunEventKind
from niki_agent.niki.turn_summary import TurnSummary, TurnSummaryData

if TYPE_CHECKING:
    from collections.abc import Callable, Coroutine


def _event(sequence: int, kind: RunEventKind, **payload: Any) -> RunEvent:
    return RunEvent(
        schema_version=1,
        event_id=f"e{sequence}",
        session_id="s1",
        turn_id="t1",
        sequence=sequence,
        kind=kind,
        payload=payload,
    )


def test_turn_summary_reports_verified_counts_and_duration():
    events = [
        _event(1, RunEventKind.turn_started),
        _event(
            2,
            RunEventKind.tool_started,
            call_id="write-1",
            tool_name="write_file",
            file_id="file-a",
        ),
        _event(
            3,
            RunEventKind.tool_finished,
            call_id="write-1",
            tool_name="write_file",
            status="success",
        ),
        _event(
            4,
            RunEventKind.tool_started,
            call_id="edit-1",
            tool_name="edit_file",
            file_id="file-a",
        ),
        _event(
            5,
            RunEventKind.tool_finished,
            call_id="edit-1",
            tool_name="edit_file",
            status="success",
        ),
        _event(
            6,
            RunEventKind.tool_started,
            call_id="read-1",
            tool_name="read_file",
        ),
        _event(
            7,
            RunEventKind.tool_finished,
            call_id="read-1",
            tool_name="read_file",
            status="success",
            file_id="should-not-count",
        ),
        _event(
            8,
            RunEventKind.tool_started,
            call_id="write-2",
            tool_name="write_file",
            file_id="file-b",
        ),
        _event(
            9,
            RunEventKind.tool_finished,
            call_id="write-2",
            tool_name="write_file",
            status="error",
        ),
        _event(
            10,
            RunEventKind.tool_finished,
            call_id="write-1",
            tool_name="write_file",
            status="success",
        ),
        _event(11, RunEventKind.turn_finished),
    ]
    data = TurnSummary.from_events(events, 1.5)
    assert data.tool_count == 4
    assert data.changed_file_count == 1
    assert data.status == "completed"
    assert data.elapsed_seconds == pytest.approx(1.5)


@pytest.mark.parametrize(
    ("kind", "status"),
    [
        (RunEventKind.turn_finished, "completed"),
        (RunEventKind.turn_failed, "failed"),
        (RunEventKind.turn_cancelled, "cancelled"),
    ],
)
def test_turn_summary_uses_terminal_event_status(kind: RunEventKind, status: str):
    data = TurnSummary.from_events([_event(1, kind)], 0.25)
    assert data.status == status


@pytest.mark.parametrize("elapsed", [-1.0, float("inf"), float("nan")])
def test_turn_summary_rejects_invalid_elapsed_time(elapsed: float):
    with pytest.raises(ValueError, match="elapsed"):
        TurnSummary.from_events([_event(1, RunEventKind.turn_finished)], elapsed)


async def test_niki_app_renders_summary_after_a_runtime_turn():
    """Runtime events dispatch one plain summary message through Niki's hook."""
    from unittest.mock import AsyncMock

    from niki_agent.niki.app import NikiApp

    class SummaryProbe(NikiApp):
        def __init__(self) -> None:
            """Provide only lifecycle hooks consumed by Niki's event method."""
            self._active_goal = None
            self._turn_summary_events = {}
            self._turn_summary_started_at = {}
            self.scheduled: list[Callable[[], object]] = []
            self._captured_workers: list[Coroutine[Any, Any, None]] = []
            self._mount_message = AsyncMock()
            self.call_later = cast(
                "Any",
                lambda callback, *_args, **_kwargs: (
                    self.scheduled.append(callback) or True
                ),
            )
            self.run_worker = cast(
                "Any",
                lambda coroutine, **_kwargs: self._captured_workers.append(coroutine),
            )

    app = SummaryProbe()
    events = [
        _event(1, RunEventKind.turn_started),
        _event(
            2,
            RunEventKind.tool_started,
            call_id="c1",
            tool_name="write_file",
            file_id="file-a",
        ),
        _event(3, RunEventKind.approval_required, interrupt_ids=["approval-1"]),
        _event(4, RunEventKind.turn_started),
        _event(
            5,
            RunEventKind.tool_finished,
            call_id="c1",
            tool_name="write_file",
            status="success",
        ),
        _event(6, RunEventKind.turn_finished),
    ]
    for event in events:
        app._on_runtime_event(event)
    assert len(app.scheduled) == 1
    app.scheduled[0]()
    await app._captured_workers[0]

    from niki_agent.tui.widgets.messages import AppMessage

    mount = app._mount_message
    assert isinstance(mount, AsyncMock)
    assert mount.await_args is not None
    message = mount.await_args.args[0]
    assert isinstance(message, AppMessage)
    assert message._content == "Completed in 0.0s | 1 tool | 1 file change reported"


def test_turn_summary_renderer_uses_plain_text_and_exact_counts():
    text = TurnSummary.render(
        TurnSummaryData(
            changed_file_count=2,
            tool_count=3,
            status="failed",
            elapsed_seconds=1.26,
        )
    )
    assert text == "Failed in 1.3s | 3 tools | 2 file changes reported"
