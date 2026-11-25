"""V5: the composer is ruled, glyph-prefixed, hinted, and queue-aware.

Each assertion is about what the terminal shows or what the widget renders --
not about the source of the values -- so a refactor that keeps the visible
contract can move the implementation freely.
"""

from __future__ import annotations

import asyncio
from typing import TYPE_CHECKING

from unit_tests.niki.turn_harness import (
    Gate,
    ScriptedAgent,
    Text,
    arm_turn,
    make_app,
    screen_text,
    wait_idle,
    wait_running,
)

if TYPE_CHECKING:
    from textual.pilot import Pilot

    from deepagents_code.niki.app import NikiApp

from textual.widgets import Static

from deepagents_code.tui.widgets.chat_input import ChatTextArea

SIZE = (100, 32)


def _composer(app: NikiApp) -> ChatTextArea:
    return app.query_one("#chat-input", ChatTextArea)


def _queue_label(app: NikiApp) -> Static:
    return app.query_one("#composer-queue", Static)


async def test_v5_the_prompt_glyph_is_the_shared_cursor_glyph() -> None:
    """V5: the composer glyph comes from `get_glyphs()`, per mode."""
    from deepagents_code.config import get_glyphs

    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        normal = str(app.query_one("#prompt").render())
        composer = app._chat_input
        composer.mode = "shell"
        await pilot.pause()
        shell = str(app.query_one("#prompt").render())
        composer.mode = "command"
        await pilot.pause()
        command = str(app.query_one("#prompt").render())

    assert normal == get_glyphs().cursor, (
        f"V5: normal glyph is {normal!r}, not the shared cursor glyph "
        f"{get_glyphs().cursor!r}"
    )
    assert shell == "$", f"V5: shell glyph is {shell!r}"
    assert command == "/", f"V5: command glyph is {command!r}"


async def test_v5_the_empty_composer_shows_a_placeholder_and_drops_it_on_typing() -> (
    None
):
    """V5: the hint fills the empty composer and leaves with the first key."""
    from deepagents_code.tui.widgets.chat_input import ChatInput

    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        empty_screen = screen_text(app)
        hint = _composer(app).placeholder
        await pilot.press("h")
        await pilot.pause()
        typed_screen = screen_text(app)

    assert hint == ChatInput.COMPOSER_PLACEHOLDER, (
        f"V5: the empty composer has no placeholder: {hint!r}"
    )
    assert hint in empty_screen, (
        f"V5: the placeholder never reached the screen:\n{empty_screen}"
    )
    assert hint not in typed_screen, (
        f"V5: the placeholder survived the first keystroke:\n{typed_screen}"
    )


async def test_v5_the_composer_draws_one_rule_and_no_other_border() -> None:
    """V5: the box carries a top rule only -- no second frame around it."""
    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        box = app.query_one("#input-box")
        top = box.styles.border_top
        sides = (box.styles.border_left, box.styles.border_right)
        bottom = box.styles.border_bottom
        wrapper = app.query_one("#input-area")

    assert top is not None, "V5: the composer draws no rule at all"
    assert top[0] == "tall", f"V5: the composer rule is not tall: {top!r}"
    assert all(side[0] == "" for side in sides), (
        f"V5: the composer still draws side borders: {sides!r}"
    )
    assert bottom[0] == "", f"V5: the composer still draws a bottom border: {bottom!r}"
    assert wrapper.styles.border_top[0] == "", (
        "V5: the wrapper draws a second rule above the box: "
        f"{wrapper.styles.border_top!r}"
    )


async def test_v5_a_queued_prompt_is_named_under_the_composer() -> None:
    """V5: a prompt queued mid-turn shows `N messages queued` under the input."""
    gate = asyncio.Event()
    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        agent = ScriptedAgent([Gate(gate), Text("first done")], [Text("second done")])
        await arm_turn(app, pilot, agent)
        await app._submit_input("first", "normal")
        await wait_running(app, pilot)

        assert _queue_label(app).styles.display == "none", (
            "V5: the queue banner is visible with an empty queue"
        )
        await app._submit_input("second", "normal")
        await pilot.pause()
        during = str(_queue_label(app).render())
        visible = _queue_label(app).styles.display

        gate.set()
        await wait_idle(app, pilot)
        after = str(_queue_label(app).render())
        hidden_after = _queue_label(app).styles.display

    assert during == "1 message queued", (
        f"V5: the composer does not name the queued message: {during!r}"
    )
    assert visible != "none", "V5: the queue banner stayed hidden while queued"
    assert after == "", f"V5: the queue banner outlived the queue: {after!r}"
    assert hidden_after == "none", (
        f"V5: the queue banner stayed visible after draining: {hidden_after!r}"
    )
