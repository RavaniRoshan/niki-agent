"""Framework-free session summary types."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class SessionSummary:
    session_id: str
    workspace_root: str
    title: str | None
    updated_at: str
