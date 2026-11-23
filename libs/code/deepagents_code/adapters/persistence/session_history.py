"""Adapter from existing session metadata to SessionHistoryPort."""

from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import Any

from deepagents_code.domain.session import SessionSummary
from deepagents_code.application.ports.sessions import SessionHistoryPort


def _thread_to_summary(thread: Any) -> SessionSummary:
    if isinstance(thread, SessionSummary):
        return thread
    if isinstance(thread, dict):
        return SessionSummary(
            session_id=str(thread.get("thread_id") or thread.get("session_id", "")),
            workspace_root=str(thread.get("cwd") or thread.get("workspace_root", "")),
            title=thread.get("initial_prompt") or thread.get("title"),
            updated_at=str(thread.get("updated_at") or ""),
        )
    # best-effort attribute access
    return SessionSummary(
        session_id=str(getattr(thread, "thread_id", getattr(thread, "session_id", ""))),
        workspace_root=str(getattr(thread, "cwd", getattr(thread, "workspace_root", ""))),
        title=getattr(thread, "initial_prompt", getattr(thread, "title", None)),
        updated_at=str(getattr(thread, "updated_at", "")),
    )


class SessionHistoryAdapter:
    """Implement SessionHistoryPort by delegating to existing session APIs."""

    def __init__(
        self,
        *,
        get_summary: Callable[[str], Any] | None = None,
        list_summaries: Callable[[str | None, int], Any] | None = None,
        sessions_module: Any = None,
    ) -> None:
        self._get_summary = get_summary
        self._list_summaries = list_summaries
        self._sessions = sessions_module

    def get_summary(self, session_id: str) -> SessionSummary | None:
        if self._get_summary is not None:
            result = self._get_summary(session_id)
            if result is None:
                return None
            return _thread_to_summary(result)
        if self._sessions is not None and hasattr(self._sessions, "get_thread"):
            try:
                result = self._sessions.get_thread(session_id)
            except Exception:
                result = None
            if result is None:
                return None
            return _thread_to_summary(result)
        return None

    def list_summaries(
        self, *, workspace_root: str | None, limit: int
    ) -> Sequence[SessionSummary]:
        if self._list_summaries is not None:
            results = self._list_summaries(workspace_root, limit)
            return [_thread_to_summary(r) for r in (results or [])]
        if self._sessions is not None:
            fn = getattr(self._sessions, "get_cached_threads", None)
            if fn is not None:
                try:
                    threads = fn(limit=limit)
                except TypeError:
                    threads = fn()
                if threads:
                    out = [_thread_to_summary(t) for t in threads]
                    if workspace_root is not None:
                        out = [s for s in out if s.workspace_root == workspace_root]
                    return out[:limit]
        return []
