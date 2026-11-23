"""Session history port."""

from __future__ import annotations

from collections.abc import Sequence
from typing import Protocol

from deepagents_code.domain.session import SessionSummary


class SessionHistoryPort(Protocol):
    def get_summary(self, session_id: str) -> SessionSummary | None:
        ...

    def list_summaries(
        self, *, workspace_root: str | None, limit: int
    ) -> Sequence[SessionSummary]:
        ...
