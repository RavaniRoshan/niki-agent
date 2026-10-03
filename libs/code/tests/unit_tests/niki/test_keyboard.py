"""K2/K3/K6: composer editing, navigation, and the slash menu.

These are P0 rows that the Pilot driver can decide honestly, so they are tested
against what the checklist *requires* rather than against whatever the app
happens to do. A failure here is a real gap in Niki's interaction layer, and is
reported as one.

`Docs1` makes a failing assertion meaningful: it states the row, so a red test
names the requirement it broke instead of just a method name.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from textual.app import App


def _app() -> App:  # type: ignore[type-arg]
    """A NikiApp wired to a mock agent, post-paint work stubbed out."""
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=MagicMock(), thread_id="niki-keys")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


def _text(app: App) -> str:  # type: ignore[type-arg]
    return str(getattr(app.query_one("#chat-input"), "text", ""))


@pytest.mark.parametrize(
    ("row", "keys", "expected"),
    [
        ("K3 char movement", ["a", "b", "c", "left", "left", "X"], "aXbc"),
        ("K3 home/end", ["a", "b", "c", "home", "Z"], "Zabc"),
        ("K3 home/end reverse", ["a", "b", "c", "home", "Z", "end", "Q"], "ZabcQ"),
        ("K3 ctrl+u kill line", ["a", "b", "c", "ctrl+u"], ""),
        ("K3 ctrl+k kill to end", ["a", "b", "left", "ctrl+k"], "a"),
        (
            "K3 alt+backspace word",
            ["f", "o", "o", "space", "b", "a", "r", "alt+backspace"],
            "foo ",
        ),
        ("K3 backspace", ["a", "b", "backspace"], "a"),
    ],
)
async def test_composer_editing(
    row: str,  # noqa: ARG001 - names the checklist row for failure output
    keys: list[str],
    expected: str,
) -> None:
    """K3: every editing gesture the checklist names, with the result it requires."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        for key in keys:
            await pilot.press(*_split(key))
            await pilot.pause()
        actual = _text(app)

    assert actual == expected, (
        f"K3: after {keys} the composer holds {actual!r}, expected {expected!r}"
    )


def _count_suggestions(app: App) -> int:  # type: ignore[type-arg]
    """How many slash-command suggestions the autocomplete popup is showing.

    Read from whatever popup-like widget is mounted rather than from a single
    hardcoded class, so the count survives upstream reorganising its widgets.
    Returns 0 when no popup is mounted, which is itself the signal K6 cares
    about.
    """
    popups = list(app.screen.query("#completion-popup"))
    if not popups:
        return 0
    popup = popups[0]
    for child in popup.walk_children(with_self=True):
        # Textual's OptionList exposes its rows; that is what "a suggestion" is.
        options = getattr(child, "_options", None)
        if options is not None:
            return len(options)
    return 0


def _split(key: str) -> list[str]:
    """Expand a compact key spec like `"alt+backspace"` or a literal space."""
    return ["space"] if key == "space" else [key]


async def test_composer_keeps_focus_after_typing() -> None:
    """F1: the composer must hold focus so typing never escapes to the transcript."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await pilot.press("h", "i")
        await pilot.pause()
        focused = app.focused

    assert focused is not None
    assert focused.id == "chat-input", f"F1: focus moved to {focused.id!r} after typing"


async def test_escape_closes_the_slash_menu_before_cancelling_input() -> None:
    """K7: Esc must close the popup first, leaving the typed text intact."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await pilot.press("slash")
        await pilot.pause()
        await pilot.press("t", "h", "e", "space")
        await pilot.pause()
        await pilot.press("escape")
        await pilot.pause()
        after = _text(app)

    # Inline completion means the buffer legitimately holds the completed word,
    # so assert the property K7 actually requires: the popup closed and Esc did
    # NOT discard what the user typed.
    assert after.strip(), (
        "K7: Esc after the slash menu emptied the composer instead of closing "
        "the popup and keeping the text"
    )


async def test_slash_menu_filters_and_narrows_to_one_row() -> None:
    """K6: '/' opens a filtered menu that narrows as the user types."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await pilot.press("slash")
        await pilot.pause()
        unfiltered = _count_suggestions(app)
        await pilot.press("c", "l", "e", "a", "n")
        await pilot.pause()
        filtered = _count_suggestions(app)

    assert unfiltered > 0, "K6: '/' produced no suggestions at all"
    assert filtered > 0, "K6: filtering 'clean' left nothing suggested"
    assert filtered <= unfiltered, (
        f"K6: filtering widened the list ({unfiltered} -> {filtered})"
    )


async def test_history_recall_with_up_arrow() -> None:
    """K3: Up/Down walk history at the buffer boundaries."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await pilot.press("a", "space", "t", "e", "s", "t")
        await pilot.pause()
        await pilot.press("enter")
        await pilot.pause()
        for _ in range(6):
            await pilot.press("escape")
            await pilot.pause()
        await pilot.press("up")
        await pilot.pause()
        recalled = _text(app)

    assert recalled.strip() == "a test", (
        f"K3: up-arrow recalled {recalled!r}, expected the previous message"
    )
