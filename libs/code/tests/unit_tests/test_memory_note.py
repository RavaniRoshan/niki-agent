"""`#` in the chat input saves a memory note instead of sending a prompt."""

from __future__ import annotations

from typing import TYPE_CHECKING
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from niki_agent.app import DeepAgentsApp
from niki_agent.memory_note import NOTES_HEADING, append_memory_note, memory_note_lines
from niki_agent.tui.widgets.chat_input import ChatInput

if TYPE_CHECKING:
    from collections.abc import Iterator
    from pathlib import Path


def _text(path: Path) -> str:
    """Read a memory file from an async test without the blocking-call rule."""
    return path.read_text(encoding="utf-8")


def _exists(path: Path) -> bool:
    """Check for a memory file from an async test without the blocking rule."""
    return path.exists()


def test_note_lines_skip_blanks() -> None:
    assert memory_note_lines("one\n\n  two  \n") == ["- one", "- two"]


def test_note_lines_reject_empty_text() -> None:
    assert memory_note_lines("   \n") == []


def test_append_creates_file_and_heading(tmp_path: Path) -> None:
    path = tmp_path / "AGENTS.md"
    append_memory_note("prefer ruff", path)
    assert path.read_text(encoding="utf-8") == f"{NOTES_HEADING}\n- prefer ruff\n"


def test_append_uses_existing_heading(tmp_path: Path) -> None:
    path = tmp_path / "AGENTS.md"
    path.write_text("# Profile\n\n## Notes\n- first\n\n## Commands\n- make\n", "utf-8")
    append_memory_note("second", path)
    text = path.read_text(encoding="utf-8")
    assert text.index("- first") < text.index("- second") < text.index("## Commands")


def test_append_without_heading_keeps_existing_sections(tmp_path: Path) -> None:
    path = tmp_path / "AGENTS.md"
    path.write_text("# Profile\n\n## Commands\n- make\n", "utf-8")
    append_memory_note("later", path)
    text = path.read_text(encoding="utf-8")
    assert text.startswith("# Profile")
    assert text.index("## Commands") < text.index(NOTES_HEADING)


def test_empty_note_is_rejected(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="at least one"):
        append_memory_note("  ", tmp_path / "AGENTS.md")


@pytest.fixture
def memory_file(tmp_path: Path) -> Iterator[Path]:
    """Point the app's profile memory at a scratch file."""
    target = tmp_path / "AGENTS.md"
    with patch("niki_agent.app.get_user_agent_md_path", return_value=target):
        yield target


async def test_hash_note_never_reaches_the_agent(memory_file: Path) -> None:
    app = DeepAgentsApp(agent=MagicMock())
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    submit = AsyncMock()
    async with app.run_test() as pilot:
        with patch.object(app, "_submit_input", submit):
            await pilot.pause()
            await app.on_chat_input_submitted(
                ChatInput.Submitted("# prefer ruff over flake8", "normal")
            )
    submit.assert_not_awaited()
    assert _text(memory_file).endswith("- prefer ruff over flake8\n")


async def test_bare_hash_writes_nothing(memory_file: Path) -> None:
    app = DeepAgentsApp(agent=MagicMock())
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    submit = AsyncMock()
    async with app.run_test() as pilot:
        with patch.object(app, "_submit_input", submit):
            await pilot.pause()
            await app.on_chat_input_submitted(ChatInput.Submitted("#  ", "normal"))
    submit.assert_not_awaited()
    assert not _exists(memory_file)
