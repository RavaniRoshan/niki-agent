"""M1/M2/M5 and F4: mouse, scroll anchoring, and keyboard equivalents.

These rows are about behaviour under a real pointer and a real scrollback, which
the Pilot driver can drive. The transcript is filled before each test: with a
short transcript nothing scrolls, and a scroll test that cannot scroll passes
for the wrong reason.

Two rows here were once **characterised defects** and are now closed:
`pageup`/`pagedown` reach the transcript from the composer, and a scrollable
draft or an open modal keeps the key. See `test_pageup_*`.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from textual.app import App
    from textual.pilot import Pilot

TRANSCRIPT_LINES = 60
FILLED_SIZES = [(80, 24), (120, 38)]


def _app() -> App:  # type: ignore[type-arg]
    """A NikiApp wired to a mock agent, post-paint work stubbed out."""
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=MagicMock(), thread_id="niki-mouse")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


async def _fill(app: App, pilot: Pilot[None]) -> None:
    """Mount enough messages that the transcript actually overflows.

    Takes the Pilot, not the App: `App` has no `pause()`, and pausing through
    the wrong object silently does nothing -- which would leave the transcript
    too short to scroll and make every scroll assertion vacuous.
    """
    from deepagents_code.tui.widgets.messages import AssistantMessage

    await app.query_one("#messages").mount_all(
        [AssistantMessage(content=f"line {i}") for i in range(TRANSCRIPT_LINES)]
    )
    await pilot.pause()
    await pilot.pause()


@pytest.mark.parametrize(
    "size", FILLED_SIZES, ids=[f"{w}x{h}" for w, h in FILLED_SIZES]
)
async def test_m1_the_transcript_is_genuinely_scrollable(size: tuple[int, int]) -> None:
    """M1: a full transcript must actually overflow its viewport."""
    app = _app()
    async with app.run_test(size=size) as pilot:
        await pilot.pause()
        await _fill(app, pilot)
        max_scroll = app.query_one("#chat").max_scroll_y

    assert max_scroll > 0, (
        f"M1: {TRANSCRIPT_LINES} messages at {size[0]}x{size[1]} produced no "
        f"scroll range (max_scroll_y={max_scroll})"
    )


async def test_scroll_position_moves_and_sticks() -> None:
    """M1: scrolling away must take, and stay taken while the view is idle."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await _fill(app, pilot)
        chat = app.query_one("#chat")

        chat.scroll_to(y=8, animate=False)
        await pilot.pause()
        moved = chat.scroll_offset.y
        for _ in range(3):
            await pilot.pause()
        settled = chat.scroll_offset.y

    assert moved == 8, f"M1: scroll_to(8) landed at {moved}"
    assert settled == moved, (
        f"M1: the view drifted from {moved} to {settled} while idle; scrolling "
        "away is being snapped back to the bottom"
    )


async def test_f4_scroll_position_survives_a_streaming_update() -> None:
    """F4: content arriving must not yank the user back to the bottom.

    This is the row that most often regresses in a chat UI: the transcript
    auto-scrolls on every append, so a user reading history gets dragged down
    mid-thought.
    """
    import asyncio

    from deepagents_code.tui.widgets.messages import AssistantMessage

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await _fill(app, pilot)
        chat = app.query_one("#chat")
        chat.scroll_to(y=10, animate=False)
        await pilot.pause()
        before = chat.scroll_offset.y

        target = AssistantMessage(content="")
        await app.query_one("#messages").mount(target)
        await pilot.pause()
        for _ in range(20):
            await target.append_content("streamed ")
            await asyncio.sleep(0.01)
        await pilot.pause()
        after = chat.scroll_offset.y

    assert before > 0, "the test could not scroll away, so it proved nothing"
    assert after == before, (
        f"F4: streaming moved the scroll position from {before} to {after}; the "
        "user was dragged away from what they were reading"
    )


async def test_pageup_pages_the_transcript_from_the_composer() -> None:
    """K2: PgUp/PgDn reach the transcript while the composer holds focus.

    `TextArea` binds `pageup`/`pagedown` itself, so before Niki routed them at
    the app level the composer swallowed both and the transcript never moved.
    """
    from deepagents_code.niki.keymap import build_keymap

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await _fill(app, pilot)
        bound = [e for e in build_keymap(app) if e.key in {"pageup", "pagedown"}]
        chat = app.query_one("#chat")
        # The transcript is bottom-anchored while idle at the top offset 0, so
        # start from a known mid-history position the way a reader would.
        chat.scroll_to(y=10, animate=False)
        await pilot.pause()
        before = chat.scroll_offset.y
        assert before > 0, "the transcript never scrolled, so the test is vacuous"

        await pilot.press("pageup")
        await pilot.pause()
        up = chat.scroll_offset.y

        await pilot.press("pagedown")
        await pilot.pause()
        down = chat.scroll_offset.y

        focused = app.focused

    assert len(bound) >= 2, "pageup/pagedown are missing from the keymap registry"
    assert up < before, f"K2: pageup left the transcript at {up} (was {before})"
    assert down > up, f"K2: pagedown left the transcript at {down} (was {up})"
    assert focused is not None, "K2: paging left no widget focused"
    assert focused.id == "chat-input", "K2: paging moved focus off the composer"


async def test_pageup_is_left_to_a_scrollable_draft() -> None:
    """K2: a draft that scrolls keeps PgUp for itself."""
    from deepagents_code.tui.widgets.chat_input import ChatTextArea

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await _fill(app, pilot)
        composer = app.query_one("#chat-input", ChatTextArea)
        composer.text = "\n".join(f"draft line {i}" for i in range(40))
        composer.move_cursor((39, 0))
        await pilot.pause()
        chat = app.query_one("#chat")
        chat.scroll_to(y=10, animate=False)
        await pilot.pause()
        before = chat.scroll_offset.y
        cursor_before = composer.cursor_location[0]
        assert composer.max_scroll_y > 0, "the draft does not scroll"

        await pilot.press("pageup")
        await pilot.pause()

        after = chat.scroll_offset.y
        cursor_after = composer.cursor_location[0]

    assert before > 0
    assert after == before, (
        "K2: pageup paged the transcript even though the draft still scrolls"
    )
    assert cursor_after < cursor_before, (
        "K2: pageup reached the transcript instead of the draft's own text area"
    )


async def test_pageup_is_left_to_an_open_modal() -> None:
    """K2: an open modal keeps the keyboard."""
    from textual.screen import ModalScreen

    class _Modal(ModalScreen[None]):
        """A modal with no pageup binding of its own."""

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await _fill(app, pilot)
        chat = app.query_one("#chat")
        chat.scroll_to(y=10, animate=False)
        await pilot.pause()
        before = chat.scroll_offset.y
        app.push_screen(_Modal())
        await pilot.pause()

        await pilot.press("pageup")
        await pilot.pause()

        after = chat.scroll_offset.y

    assert before > 0
    assert after == before, "K2: pageup paged the transcript behind an open modal"


@pytest.mark.parametrize(
    "size", FILLED_SIZES, ids=[f"{w}x{h}" for w, h in FILLED_SIZES]
)
async def test_m2_clicking_focuses_the_composer(size: tuple[int, int]) -> None:
    """M2: a click in the input area must land focus on the composer."""
    app = _app()
    async with app.run_test(size=size) as pilot:
        await pilot.pause()
        await pilot.click("#input-area")
        await pilot.pause()
        focused = app.focused

    assert focused is not None
    assert focused.id == "chat-input", (
        f"M2: clicking the input area left focus on {focused.id!r}"
    )


async def test_m5_every_mouse_gesture_has_a_keyboard_equivalent() -> None:
    """M5: no action may be reachable only by pointer.

    Built from the keymap registry rather than a hand-written list, so a binding
    added upstream is covered automatically instead of silently going untested.
    """
    from deepagents_code.niki.keymap import build_keymap

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        keys = {e.key for e in build_keymap(app)}

    equivalents = {
        "focus_composer": {"tab"},
        "scroll_up": {"pageup", "up"},
        "scroll_down": {"pagedown", "down"},
        "jump_to_bottom": {"end", "g"},
        "cancel": {"escape"},
        "submit": {"enter"},
    }
    missing = {
        gesture: sorted(candidates)
        for gesture, candidates in equivalents.items()
        if not (candidates & keys)
    }
    assert not missing, (
        f"M5: these gestures have no key binding in the registry: {missing}"
    )
