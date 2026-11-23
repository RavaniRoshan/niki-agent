"""Tests for Niki goal status projection."""

from __future__ import annotations

from deepagents_code.domain.events import GoalProgressEvent


def test_goal_status_projection():
    e = GoalProgressEvent(session_id="s1", status="active", note="working")
    assert e.status == "active"
