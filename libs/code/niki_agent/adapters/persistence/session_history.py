"""Adapter from existing session metadata to SessionHistoryPort."""

from __future__ import annotations

from typing import TYPE_CHECKING, cast

from niki_agent.domain.session import SessionSummary

if TYPE_CHECKING:
    from collections.abc import Callable, Sequence


def _title(thread: object) -> str | None:
    """Read an optional title from a session record.

    Returns:
        The title when present and textual, otherwise None.
    """
    if isinstance(thread, dict):
        value = thread.get("initial_prompt") or thread.get("title")
    else:
        value = getattr(thread, "initial_prompt", getattr(thread, "title", None))
    return value if isinstance(value, str) else None


def _thread_to_summary(thread: object) -> SessionSummary:
    if isinstance(thread, SessionSummary):
        return thread
    if isinstance(thread, dict):
        return SessionSummary(
            session_id=str(thread.get("thread_id") or thread.get("session_id", "")),
            workspace_root=str(thread.get("cwd") or thread.get("workspace_root", "")),
            title=_title(thread),
            updated_at=str(thread.get("updated_at") or ""),
        )
    # best-effort attribute access
    return SessionSummary(
        session_id=str(getattr(thread, "thread_id", getattr(thread, "session_id", ""))),
        workspace_root=str(
            getattr(thread, "cwd", getattr(thread, "workspace_root", ""))
        ),
        title=_title(thread),
        updated_at=str(getattr(thread, "updated_at", "")),
    )


class SessionHistoryAdapter:
    """Implement SessionHistoryPort by delegating to existing session APIs."""

    def __init__(
        self,
        *,
        get_summary: Callable[[str], object] | None = None,
        list_summaries: Callable[[object, int], object] | None = None,
        sessions_module: object = None,
    ) -> None:
        """Bind optional session metadata sources."""
        self._get_summary = get_summary
        self._list_summaries = list_summaries
        self._sessions = sessions_module

    def get_summary(self, session_id: str) -> SessionSummary | None:
        """Read one summary from the configured session source.

        Returns:
            The summary, or None when the session is absent.
        """
        if self._get_summary is not None:
            result = self._get_summary(session_id)
            if result is None:
                return None
            return _thread_to_summary(result)
        getter = getattr(self._sessions, "get_thread", None)
        if callable(getter):
            try:
                result = getter(session_id)
            except (AttributeError, OSError, TypeError, ValueError):
                result = None
            if result is None:
                return None
            return _thread_to_summary(result)
        return None

    def list_summaries(
        self, *, workspace_root: str | None, limit: int
    ) -> Sequence[SessionSummary]:
        """List recent summaries, optionally filtered by workspace.

        Returns:
            Matching summaries, newest first.
        """
        if self._list_summaries is not None:
            results = self._list_summaries(workspace_root, limit)
            records = cast("list[object]", results or [])
            return [_thread_to_summary(record) for record in records]
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
