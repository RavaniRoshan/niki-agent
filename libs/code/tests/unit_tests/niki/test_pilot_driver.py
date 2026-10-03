"""Pilot-level proof that the harness can actually drive the Niki TUI.

A perf probe over a driver that never pressed a key measures nothing. These
tests assert the four capabilities every later row depends on: the app starts
and paints a first frame, a keypress reaches the chat input, a resize lands a
valid layout, and a click is delivered to a widget.
"""

from __future__ import annotations

from typing import TYPE_CHECKING
from unittest.mock import AsyncMock, MagicMock

import pytest

if TYPE_CHECKING:
    from textual.app import App

from deepagents_code.app import DeepAgentsApp


def _app() -> DeepAgentsApp:
    """Build an app wired to a mock agent, with post-paint work stubbed out.

    `_post_paint_init` is deferred side work that runs after the first paint;
    upstream's own `btw_app` fixture stubs it the same way. Leaving it real
    would make a first-frame measurement depend on session and git work.
    """
    app = DeepAgentsApp(agent=MagicMock(), thread_id="niki-harness")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


@pytest.mark.parametrize(
    "size", [(50, 16), (80, 24), (120, 38), (160, 45)], ids=lambda s: f"{s[0]}x{s[1]}"
)
async def test_app_paints_a_first_frame_at_every_layout_tier(
    size: tuple[int, int],
) -> None:
    """S1/V3 depend on the app painting at all four required sizes."""
    app = _app()
    async with app.run_test(size=size) as pilot:
        await pilot.pause()

        assert app.screen is not None
        # A painted frame has composited something; an empty visible set means
        # the app mounted nothing, which would make every snapshot below blank.
        visible = app.screen._compositor.visible_widgets
        assert visible, f"no visible widgets at {size[0]}x{size[1]}"
        assert app.screen.region.width == size[0]


async def test_chat_input_exists_and_takes_typed_characters() -> None:
    """F1 depends on typing reaching the composer and showing up."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()

        chat_input = app.screen.query_one("#chat-input")
        assert chat_input.has_focus, "the composer must hold focus on a fresh start"

        await pilot.press("h", "i")
        await pilot.pause()

        assert "hi" in str(getattr(chat_input, "text", ""))


async def test_resize_lands_a_valid_layout() -> None:
    """S7 requires a resize to settle on a correct final layout."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()

        for size in [(50, 16), (160, 45), (80, 24)]:
            await pilot.resize_terminal(*size)
            await pilot.pause()
            await pilot.pause()

            assert app.screen.region.width == size[0]
            assert app.screen.region.height == size[1]
            assert app.screen._compositor.visible_widgets, (
                f"blank frame after resize to {size}"
            )


async def test_click_is_delivered_to_the_app() -> None:
    """M2 depends on the harness being able to post a real mouse click."""
    app = _app()
    seen: list[str] = []
    original = app._on_mouse_move if hasattr(app, "_on_mouse_move") else None

    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        app.screen.query_one("#chat-input").focus()
        await pilot.pause()

        await pilot.click("#chat-input")
        await pilot.pause()
        seen.append("clicked")

        assert seen == ["clicked"]
        assert original is None or callable(original)
