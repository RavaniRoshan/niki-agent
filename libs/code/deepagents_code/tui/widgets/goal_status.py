"""Persistent inline display for the current goal."""

from __future__ import annotations

from typing import TYPE_CHECKING

from textual.content import Content
from textual.widgets import Static

from deepagents_code.config import get_glyphs
from deepagents_code.domain.events import GoalProgressEvent, RunEvent

if TYPE_CHECKING:
    from deepagents_code.resume_state import GoalStatus


class GoalStatusPanel(Static):
    """Keep the current goal and lifecycle state visible above the input."""

    def __init__(self, *, id: str | None = None) -> None:  # noqa: A002
        """Initialize an empty hidden goal panel."""
        super().__init__("", id=id, classes="goal-status-panel")
        self.display = False

    def set_goal(
        self,
        objective: str | None,
        status: GoalStatus | None,
        note: str | None,
    ) -> None:
        """Render the current goal or hide the panel when no goal exists.

        Args:
            objective: Persisted goal objective, if set.
            status: Current lifecycle state.
            note: Blocker or completion note associated with the state.
        """
        if not objective:
            self.update("")
            self.display = False
            return

        current = status or "active"
        label = "completed" if current == "complete" else current
        content = Content.from_markup(
            "[bold]Goal $separator $status[/bold]\n$objective",
            separator=get_glyphs().separator,
            status=label,
            objective=objective,
        )
        if note and current in {"blocked", "complete"}:
            content += Content.from_markup("\n[dim]$note[/dim]", note=note)
        self.update(content)
        self.display = True

    def apply_event(
        self,
        event: RunEvent,
        *,
        session_id: str,
        objective: str | None,
    ) -> bool:
        """Apply a matching, validated goal-progress event.

        Events from a previous or different session are ignored. The objective
        remains sourced from the app's checkpoint projection; events carry only
        lifecycle status and its short note.

        Returns:
            Whether the event matched the active session and was applied.
        """
        if event.session_id != session_id:
            return False
        progress = GoalProgressEvent.from_run_event(event)
        self.apply_progress(progress, objective=objective)
        return True

    def apply_progress(
        self,
        progress: GoalProgressEvent,
        *,
        objective: str | None,
    ) -> None:
        """Render a lifecycle projection after its owning layer validates it."""
        self.set_goal(objective, progress.status, progress.note or None)
