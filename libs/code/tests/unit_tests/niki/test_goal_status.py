"""Niki goal progress event presentation tests."""

from __future__ import annotations

from unittest.mock import AsyncMock, MagicMock

import pytest

from deepagents_code.app import DeepAgentsApp
from deepagents_code.domain.events import GoalProgressEvent, RunEvent, RunEventKind
from deepagents_code.tui.textual_adapter import TextualUIAdapter
from deepagents_code.tui.widgets.goal_status import GoalStatusPanel


def _event(
    *, session_id: str = "session-1", status: str = "blocked", note: str = "waiting"
) -> RunEvent:
    return RunEvent(
        schema_version=1,
        event_id="turn-1:2",
        session_id=session_id,
        turn_id="turn-1",
        sequence=2,
        kind=RunEventKind.goal_progress,
        payload={"status": status, "note": note},
    )


def test_goal_progress_event_requires_known_status():
    with pytest.raises(ValueError, match="status"):
        GoalProgressEvent(session_id="session-1", status="finished", note="done")


def test_goal_status_panel_applies_matching_session_event():
    panel = GoalStatusPanel()

    applied = panel.apply_event(
        _event(), session_id="session-1", objective="add API credentials"
    )

    assert applied is True
    assert panel.display is True
    rendered = str(panel.render())
    assert "blocked" in rendered
    assert "add API credentials" in rendered
    assert "waiting" in rendered


def test_goal_status_panel_ignores_event_from_another_session():
    panel = GoalStatusPanel()
    panel.set_goal("current goal", "active", None)
    before = str(panel.render())

    applied = panel.apply_event(
        _event(session_id="other-session"),
        session_id="session-1",
        objective="current goal",
    )

    assert applied is False
    assert str(panel.render()) == before


def test_goal_status_panel_rejects_unverified_completion_event():
    panel = GoalStatusPanel()
    event = RunEvent(
        schema_version=1,
        event_id="turn-1:2",
        session_id="session-1",
        turn_id="turn-1",
        sequence=2,
        kind=RunEventKind.goal_progress,
        payload={"status": "complete", "note": "done"},
    )

    with pytest.raises(ValueError, match="checkpoint verification"):
        panel.apply_event(event, session_id="session-1", objective="goal")


def test_textual_adapter_forwards_goal_and_compaction_events():
    received: list[RunEvent] = []
    adapter = TextualUIAdapter(
        mount_message=lambda *_args, **_kwargs: None,
        update_status=lambda _status: None,
        request_approval=lambda *_args, **_kwargs: None,
        on_runtime_event=received.append,
    )

    goal_event = _event()
    compaction_event = RunEvent(
        schema_version=1,
        event_id="turn-1:3",
        session_id="session-1",
        turn_id="turn-1",
        sequence=3,
        kind=RunEventKind.context_compacted,
        payload={"cutoff_index": 7},
    )
    adapter.handle_runtime_event(goal_event)
    adapter.handle_runtime_event(compaction_event)

    assert received == [goal_event, compaction_event]


@pytest.mark.asyncio
async def test_completion_projection_waits_for_checkpoint_persistence():
    app = DeepAgentsApp(agent=MagicMock())
    panel = GoalStatusPanel()
    projected: list[GoalProgressEvent] = []
    apply_progress = panel.apply_progress

    def capture(progress: GoalProgressEvent, *, objective: str | None) -> None:
        projected.append(progress)
        apply_progress(progress, objective=objective)

    panel.apply_progress = capture
    app._goal_status_panel = panel
    app._lc_thread_id = "session-1"
    app._active_goal = "add API credentials"
    app._goal_status = "active"
    app._active_rubric = "all tests pass"
    app._aupdate_thread_state = AsyncMock(
        side_effect=[RuntimeError("write failed"), None]
    )
    app.notify = MagicMock()
    app._mount_message = AsyncMock()

    first = await app._commit_pending_goal_completion(
        "all criteria passed", previous_status="active"
    )
    assert first is False
    assert projected == []
    assert app._goal_status == "active"

    second = await app._commit_pending_goal_completion(
        "all criteria passed", previous_status="active"
    )
    assert second is True
    assert projected == [
        GoalProgressEvent(
            session_id="session-1",
            status="complete",
            note="all criteria passed",
        )
    ]
    assert "completed" in str(panel.render())
