"""Tests for the session history adapter."""

from __future__ import annotations

from deepagents_code.adapters.persistence.session_history import SessionHistoryAdapter
from deepagents_code.domain.session import SessionSummary


def test_session_history_adapter_reads_existing_session_metadata():
    def get_summary(sid):
        return {"thread_id": sid, "cwd": "/w", "initial_prompt": "hi", "updated_at": "2026-01-01T00:00:00Z"}

    def list_summaries(workspace_root, limit):
        return [
            {"thread_id": "t1", "cwd": "/w", "initial_prompt": "hi", "updated_at": "2026-01-01T00:00:00Z"},
        ]

    adapter = SessionHistoryAdapter(get_summary=get_summary, list_summaries=list_summaries)
    s = adapter.get_summary("t1")
    assert isinstance(s, SessionSummary)
    assert s.session_id == "t1"
    assert s.workspace_root == "/w"
    lst = adapter.list_summaries(workspace_root="/w", limit=10)
    assert len(lst) == 1
    assert lst[0].session_id == "t1"
