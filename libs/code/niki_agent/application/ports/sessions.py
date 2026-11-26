"""Session history port."""

from __future__ import annotations

from typing import TYPE_CHECKING, Protocol

if TYPE_CHECKING:
    from collections.abc import Sequence

    from niki_agent.domain.session import SessionSummary


class SessionHistoryPort(Protocol):
    """Read-only boundary for session summaries."""

    def get_summary(self, session_id: str) -> SessionSummary | None:
        """Return one session summary, or None when absent."""
        ...

    def list_summaries(
        self, *, workspace_root: str | None, limit: int
    ) -> Sequence[SessionSummary]:
        """List recent summaries, optionally filtered by workspace."""
        ...
