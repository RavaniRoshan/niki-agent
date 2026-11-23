"""Tests for turn summary."""

from __future__ import annotations

from deepagents_code.domain.events import RunEvent, RunEventKind
from deepagents_code.niki.turn_summary import TurnSummary


def test_turn_summary_reports_verified_counts_and_duration():
    events = [
        RunEvent(schema_version=1, event_id="e1", session_id="s1", turn_id="t1", sequence=1, kind=RunEventKind.turn_started, payload={}),
        RunEvent(schema_version=1, event_id="e2", session_id="s1", turn_id="t1", sequence=2, kind=RunEventKind.tool_finished, payload={"path": "a.py"}),
        RunEvent(schema_version=1, event_id="e3", session_id="s1", turn_id="t1", sequence=3, kind=RunEventKind.turn_finished, payload={}),
    ]
    data = TurnSummary.from_events(events, 1.5)
    assert data.tool_count == 1
    assert data.changed_file_count == 1
    assert data.status == "completed"
    assert data.elapsed_seconds == 1.5
