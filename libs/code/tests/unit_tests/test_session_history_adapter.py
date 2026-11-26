"""Tests for the session history adapter."""

from __future__ import annotations

from niki_agent.adapters.persistence.session_history import SessionHistoryAdapter
from niki_agent.domain.session import SessionSummary


def test_session_history_adapter_reads_existing_session_metadata():
    def get_summary(sid) -> dict[str, str]:
        return {
            "thread_id": sid,
            "cwd": "/w",
            "initial_prompt": "hi",
            "updated_at": "2026-01-01T00:00:00Z",
        }

    def list_summaries(_workspace_root, _limit) -> list[dict[str, str]]:
        return [
            {
                "thread_id": "t1",
                "cwd": "/w",
                "initial_prompt": "hi",
                "updated_at": "2026-01-01T00:00:00Z",
            },
        ]

    adapter = SessionHistoryAdapter(
        get_summary=get_summary, list_summaries=list_summaries
    )
    summary = adapter.get_summary("t1")
    assert isinstance(summary, SessionSummary)
    assert summary.session_id == "t1"
    assert summary.workspace_root == "/w"
    listed = adapter.list_summaries(workspace_root="/w", limit=10)
    assert len(listed) == 1
    assert listed[0].session_id == "t1"
