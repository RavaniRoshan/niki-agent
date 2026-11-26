"""F5: the session picker navigates and resumes against a real thread store.

The earlier F5 row only proved the screen could be imported. This exercises the
path a user takes: a real SQLite checkpoint store on disk, the app's own
`/threads` handler, arrow keys through the modal, Enter, and the resume call.

Nothing about the app is stubbed except the two edges that need a live server:
`_post_paint_init` (no provider) and `_resume_same_agent_thread` (no HTTP).
"""

from __future__ import annotations

import asyncio
import json
import sqlite3
from pathlib import Path
from typing import TYPE_CHECKING

import pytest

from unit_tests.niki.turn_harness import make_app

if TYPE_CHECKING:
    from deepagents_code.niki.app import NikiApp

CWD = str(Path.cwd())
"""Threads are filtered to the current working directory by default."""

THREADS = (
    ("thread-oldest", "2026-01-01T00:00:00+00:00"),
    ("thread-middle", "2026-01-02T00:00:00+00:00"),
    ("thread-newest", "2026-01-03T00:00:00+00:00"),
)


def _write_sessions_db(state_dir: Path) -> None:
    """Create a minimal but real `checkpoints` store the app will query."""
    state_dir.mkdir(parents=True, exist_ok=True)
    con = sqlite3.connect(state_dir / "sessions.db")
    con.execute(
        "CREATE TABLE checkpoints (thread_id TEXT, checkpoint_id TEXT, metadata TEXT)"
    )
    for thread_id, updated_at in THREADS:
        metadata = json.dumps(
            {
                "agent_name": "build",
                "updated_at": updated_at,
                "created_at": updated_at,
                "git_branch": "main",
                "cwd": CWD,
            }
        )
        con.execute(
            "INSERT INTO checkpoints VALUES (?, ?, ?)",
            (thread_id, f"cp-{thread_id}", metadata),
        )
    con.commit()
    con.close()


@pytest.fixture
def sessions_db(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """Point the session store at a throwaway directory with three threads."""
    from deepagents_code import model_config, sessions

    state_dir = tmp_path / "state"
    _write_sessions_db(state_dir)
    monkeypatch.setattr(model_config, "DEFAULT_STATE_DIR", state_dir)
    monkeypatch.setattr(sessions, "_db_path", None)
    monkeypatch.setattr(sessions, "_recent_threads_cache", {})
    return state_dir


@pytest.mark.usefixtures("sessions_db")
async def test_f5_arrow_keys_resume_the_thread_under_the_cursor(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """F5: three stored threads, two Downs, Enter -> the bottom row resumes."""
    from deepagents_code.tui.widgets.thread_selector import ThreadSelectorScreen

    app: NikiApp = make_app()
    resumed: list[str] = []

    async def _record(thread_id: str) -> None:
        await asyncio.sleep(0)
        resumed.append(thread_id)

    monkeypatch.setattr(app, "_resume_same_agent_thread", _record)

    async with app.run_test(size=(120, 40)) as pilot:
        await pilot.pause()
        await app._show_thread_selector()
        await pilot.pause()
        await app.workers.wait_for_complete()
        await pilot.pause()

        screen = app.screen
        assert isinstance(screen, ThreadSelectorScreen), (
            f"F5: /threads did not open the picker; got {type(screen).__name__}"
        )
        visible = [row["thread_id"] for row in screen._filtered_threads]
        assert visible == ["thread-newest", "thread-middle", "thread-oldest"], (
            f"F5: rows were not read from the store newest-first: {visible}"
        )

        await pilot.press("down", "down")
        await pilot.pause()
        await pilot.press("enter")
        await pilot.pause()
        await app.workers.wait_for_complete()
        await pilot.pause()

    assert resumed == ["thread-oldest"], (
        f"F5: Enter did not resume the third row; resumed={resumed}"
    )
