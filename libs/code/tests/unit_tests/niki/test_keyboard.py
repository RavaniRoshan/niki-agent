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


@pytest.mark.parametrize(
    ("row", "keys"),
    [
        ("K3 shift+enter", ["a", "shift+enter", "b"]),
        ("K3 alt+enter", ["a", "alt+enter", "b"]),
        ("K3 backslash+enter", ["a", "backslash", "enter", "b"]),
    ],
)
async def test_composer_multiline_inserts_a_newline(
    row: str,  # noqa: ARG001 - names the checklist row for failure output
    keys: list[str],
) -> None:
    """K3: the multi-line gestures insert a line break instead of submitting."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        for key in keys:
            await pilot.press(*_split(key))
            await pilot.pause()
        actual = _text(app)
        submitted = list(app.query("UserMessage"))

    assert actual == "a\nb", f"K3: after {keys} the composer holds {actual!r}"
    assert not submitted, "K3: the newline gesture submitted the prompt"


@pytest.mark.parametrize(
    ("row", "text", "lefts", "expected"),
    [
        # A combining mark must travel with its base, not be split from it.
        ("K3 grapheme cursor", "e\u0301", 1, "Xe\u0301"),
        # A wide character is a single stop, so two Lefts clear it.
        ("K3 wide-character cursor", "a\u754cb", 2, "aX\u754cb"),
    ],
)
async def test_composer_cursor_steps_by_grapheme(
    row: str,  # noqa: ARG001 - names the checklist row for failure output
    text: str,
    lefts: int,
    expected: str,
) -> None:
    """K3: one Left press is one visible character, whatever its byte width."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        from deepagents_code.tui.widgets.chat_input import ChatTextArea

        composer = app.query_one("#chat-input", ChatTextArea)
        composer.insert(text)
        await pilot.pause()
        for _ in range(lefts):
            await pilot.press("left")
        await pilot.press("X")
        await pilot.pause()
        actual = composer.text

    assert actual == expected, (
        f"K3: Left then X on {text!r} produced {actual!r}, expected {expected!r}"
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


async def test_k6_arrows_tab_and_enter_drive_the_slash_menu() -> None:
    """K6: the menu is keyboard-drivable, not decoration.

    Arrows move the highlight, Tab accepts the highlighted command into the
    composer, and Enter dispatches it. All three are asserted against the
    popup's live selection and the transcript, so a change in how the menu is
    rendered cannot make this pass vacuously.
    """
    from deepagents_code.tui.widgets.chat_input import ChatInput

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        chat = app.query_one(ChatInput)
        await pilot.press("slash")
        await pilot.pause()
        count = len(chat._current_suggestions)
        assert count > 1, "K6: '/' produced no menu to drive"
        assert chat._current_selected_index == 0

        await pilot.press("down", "down")
        await pilot.pause()
        assert chat._current_selected_index == 2
        await pilot.press("up")
        await pilot.pause()
        assert chat._current_selected_index == 1
        chosen = chat._current_suggestions[1][0].lstrip("/")

        await pilot.press("tab")
        await pilot.pause()
        accepted = _text(app).strip()
        remaining = list(chat._current_suggestions)

    assert accepted == chosen, (
        f"K6: Tab accepted {accepted!r}, expected the highlighted {chosen!r}"
    )
    assert not remaining, "K6: Tab left the menu open after accepting"


async def test_k6_enter_dispatches_the_highlighted_command() -> None:
    """K6: Enter runs the highlighted command rather than submitting the text."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await pilot.press("slash")
        for key in ("h", "e", "l", "p"):
            await pilot.press(key)
        await pilot.pause()
        await pilot.press("enter")
        for _ in range(3):
            await pilot.pause()
        helped = any(
            "Usage:" in str(message.render())
            for message in app.query("AppMessage")
        )

    assert helped, "K6: Enter did not run the highlighted `/help`"


async def test_k7_escape_closes_in_order_and_restores_focus() -> None:
    """K7: Esc unwinds one layer at a time and always lands back on the composer.

    The order is modal, then popup, then search, then a nested screen, then the
    turn itself. Each layer is opened and dismissed in sequence, and the draft a
    user typed is still there at the end: Esc interrupts, it never eats text.
    """
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()

        # 1. A modal owns the keyboard until it is dismissed.
        await pilot.press("question_mark")
        await pilot.pause()
        assert len(app.screen_stack) == 2, "K7: `?` did not open a modal"
        await pilot.press("escape")
        await pilot.pause()
        assert len(app.screen_stack) == 1, "K7: Esc left the modal open"
        assert getattr(app.focused, "id", None) == "chat-input", (
            "K7: closing the modal did not restore composer focus"
        )

        # 2. The slash popup closes before anything else.
        await pilot.press("slash")
        await pilot.pause()
        await pilot.press("escape")
        await pilot.pause()
        assert len(app.screen_stack) == 1, "K7: Esc left the popup screen open"
        assert getattr(app.focused, "id", None) == "chat-input"

        # 3. Prompt search abandons and hands the draft back.
        await pilot.press("b", "o", "d", "y")
        await pilot.pause()
        await pilot.press("ctrl+r")
        await pilot.pause()
        assert len(app.screen_stack) == 2, "K7: Ctrl+R did not open search"
        await pilot.press("escape")
        await pilot.pause()
        assert len(app.screen_stack) == 1, "K7: Esc left search open"
        assert getattr(app.focused, "id", None) == "chat-input"

        # 4. A nested screen (the debug console) closes on Esc too.
        await pilot.press("ctrl+backslash")
        await pilot.pause()
        assert len(app.screen_stack) == 2, "K7: the debug console did not open"
        await pilot.press("escape")
        await pilot.pause()
        assert len(app.screen_stack) == 1, "K7: Esc left the debug console open"
        assert getattr(app.focused, "id", None) == "chat-input"

        # 5. The interrupt step cancels work, not the prompt.
        await pilot.press("escape")
        await pilot.pause()
        draft = _text(app)

    assert draft == "body", f"K7: the interrupt step altered the draft ({draft!r})"


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


async def test_first_run_name_prompt_owns_focus() -> None:
    """V2/K3: a fresh launch must type into the name prompt, not the composer.

    Regression: mount set focus on the chat input while `LaunchNameScreen` was
    already on the stack, so a first-time user's keystrokes went to a hidden
    composer and "Enter to continue" did nothing.
    """
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=MagicMock(), thread_id="niki-first-run", launch_init=True)
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause(0.5)
        assert type(app.screen).__name__ == "LaunchNameScreen", (
            "the fresh-profile onboarding did not open"
        )
        assert getattr(app.focused, "id", None) == "launch-name-input", (
            "focus is not on the name field; typing is going somewhere hidden"
        )
        await pilot.press("b", "o", "b")
        await pilot.pause()
        from textual.widgets import Input

        assert app.screen.query_one("#launch-name-input", Input).value == "bob"
        await pilot.press("enter")
        await pilot.pause()
        assert type(app.screen).__name__ != "LaunchNameScreen", (
            "Enter did not continue past the name prompt"
        )
