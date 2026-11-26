"""Append a `#` note from the chat input to the agent's memory file.

Claude Code's `#` prefix is the shortest way to say "remember this": the line
is written to memory instead of being sent as a prompt. Niki stores it in the
active agent profile's `AGENTS.md`, the same file `MemoryMiddleware` loads at
start-up, so the note is in the next turn's system prompt without restarting
the profile.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from pathlib import Path

#: Heading the notes are collected under. Matches the convention the vendored
#: memory middleware documents for hand-written sections.
NOTES_HEADING = "## Notes"

_EMPTY_NOTE_ERROR = "a memory note needs at least one non-empty line"


def memory_note_lines(text: str) -> list[str]:
    """Return the bullet lines a note appends, one per non-empty input line.

    Args:
        text: Everything after the leading `#`, already stripped by the caller.

    Returns:
        Bullet lines without trailing newlines. Empty when the note has no
        content, so callers can reject it before touching the file.
    """
    return [f"- {line.strip()}" for line in text.splitlines() if line.strip()]


def _insert_under_notes(existing: str, bullets: list[str]) -> str:
    """Return `existing` with `bullets` placed at the end of the notes section."""
    lines = existing.splitlines()
    try:
        heading = lines.index(NOTES_HEADING)
    except ValueError:
        body = existing.rstrip("\n")
        parts = [body] if body else []
        parts.append(NOTES_HEADING)
        parts.extend(bullets)
        return "\n".join(parts) + "\n"

    end = len(lines)
    for index in range(heading + 1, len(lines)):
        if lines[index].startswith("## "):
            end = index
            break
    insert_at = end
    while insert_at > heading + 1 and not lines[insert_at - 1].strip():
        insert_at -= 1
    lines[insert_at:insert_at] = bullets
    return "\n".join(lines) + "\n"


def append_memory_note(text: str, path: Path) -> list[str]:
    """Append `text` as bullets under the notes heading, creating what is missing.

    The file is edited rather than appended to, because a bare append would put
    the note under whatever section happens to be last.

    Args:
        text: Everything after the leading `#`.
        path: The agent profile's `AGENTS.md`.

    Returns:
        The bullets that were written.

    Raises:
        ValueError: When the note has no content.
    """
    bullets = memory_note_lines(text)
    if not bullets:
        raise ValueError(_EMPTY_NOTE_ERROR)

    existing = path.read_text(encoding="utf-8") if path.exists() else ""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(_insert_under_notes(existing, bullets), encoding="utf-8")
    return bullets


__all__ = ["NOTES_HEADING", "append_memory_note", "memory_note_lines"]
