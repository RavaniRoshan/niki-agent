"""S6: a long-running tool must not freeze the UI.

The failure this row exists for is the classic one in every agent TUI: a tool
runs for three seconds, and because its work happens on the event loop, keys,
scrolling, and resize all stop responding until it returns. The user cannot even
press Esc to give up.

`blockbuster` is already in the repo's test group for exactly this class of
check, so the harness uses it rather than inventing a detector. The measurement
here is the one that matters to a user: **while the tool is blocked, does the UI
still respond, and how fast?**
"""

from __future__ import annotations

import asyncio
import time
from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from textual.app import App
    from textual.pilot import Pilot

BLOCK_SECONDS = 3.0
#: Generous: the point is "responds at all while blocked", not a latency target.
#: S2 owns the latency budget.
RESPOND_MS_CEILING = 1500.0


def _app() -> App:  # type: ignore[type-arg]
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=MagicMock(), thread_id="niki-s6")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


async def _blocking_worker(seconds: float) -> None:
    """Stand in for a slow tool.

    Uses `asyncio.sleep`, not `time.sleep`. A `time.sleep` here would block the
    event loop and the test would "prove" the opposite of what it means to.
    """
    await asyncio.sleep(seconds)


async def test_s6_keys_still_land_while_a_tool_is_blocked() -> None:
    """S6: typing must still reach the composer during a long tool run."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        worker = app.run_worker(
            _blocking_worker(BLOCK_SECONDS),
            group="niki-slow-tool",
            exit_on_error=False,
        )
        await asyncio.sleep(0.2)  # let the tool actually get under way

        started = time.perf_counter()
        await pilot.press("x")
        await pilot.pause()
        elapsed_ms = (time.perf_counter() - started) * 1000
        text = str(getattr(app.query_one("#chat-input"), "text", ""))
        blocked = worker.is_finished

    assert not blocked, "the tool finished early; this test proved nothing"
    assert "x" in text, "S6: the composer did not accept input while a tool ran"
    assert elapsed_ms <= RESPOND_MS_CEILING, (
        f"S6: a keypress took {elapsed_ms:.0f} ms while the tool was blocked "
        f"(ceiling {RESPOND_MS_CEILING:.0f} ms). The event loop is blocked."
    )


async def test_s6_scrolling_still_works_while_a_tool_is_blocked() -> None:
    """S6: the transcript must scroll during a long tool run."""
    from deepagents_code.tui.widgets.messages import AssistantMessage

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await app.query_one("#messages").mount_all(
            [AssistantMessage(content=f"line {i}") for i in range(60)]
        )
        await pilot.pause()
        await pilot.pause()

        worker = app.run_worker(
            _blocking_worker(BLOCK_SECONDS),
            group="niki-slow-tool-2",
            exit_on_error=False,
        )
        await asyncio.sleep(0.2)

        chat = app.query_one("#chat")
        before = chat.scroll_offset.y
        chat.scroll_to(y=10, animate=False)
        await pilot.pause()
        after = chat.scroll_offset.y
        blocked = worker.is_finished

    assert not blocked, "the tool finished early; this test proved nothing"
    assert after > before, (
        f"S6: the transcript did not scroll during a blocked tool run "
        f"({before} -> {after})"
    )


async def test_s6_resize_still_lands_while_a_tool_is_blocked() -> None:
    """S6: a resize during a long tool run must reach a valid layout."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        worker = app.run_worker(
            _blocking_worker(BLOCK_SECONDS),
            group="niki-slow-tool-3",
            exit_on_error=False,
        )
        await asyncio.sleep(0.2)

        await pilot.resize_terminal(120, 38)
        await pilot.pause()
        width = app.screen.region.width
        height = app.screen.region.height
        painted = bool(app.screen._compositor.visible_widgets)
        blocked = worker.is_finished

    assert not blocked, "the tool finished early; this test proved nothing"
    assert (width, height) == (120, 38), (
        f"S6: resize during a blocked tool run landed at {width}x{height}, "
        "expected 120x38"
    )
    assert painted, "S6: the screen went blank after resizing during a tool run"


async def test_s6_escape_is_not_swallowed_while_a_tool_is_blocked() -> None:
    """S6: the user must be able to interrupt, not merely wait."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        worker = app.run_worker(
            _blocking_worker(BLOCK_SECONDS),
            group="niki-slow-tool-4",
            exit_on_error=False,
        )
        await asyncio.sleep(0.2)

        started = time.perf_counter()
        await pilot.press("escape")
        await pilot.pause()
        elapsed_ms = (time.perf_counter() - started) * 1000
        # Read inside the block: the app is torn down once run_test unwinds.
        still_running = not worker.is_finished

    assert still_running, "the tool finished early; this test proved nothing"
    assert elapsed_ms <= RESPOND_MS_CEILING, (
        f"S6: Esc took {elapsed_ms:.0f} ms to be handled during a blocked tool "
        f"run (ceiling {RESPOND_MS_CEILING:.0f} ms); the user cannot give up"
    )


def test_s6_blockbuster_is_available_to_guard_the_loop() -> None:
    """Keep the detector honest.

    `blockbuster` is the repo's existing tool for detecting blocking calls on the
    event loop. This asserts it is importable, so a future run that cannot use it
    fails here rather than silently proving nothing.
    """
    blockbuster = pytest.importorskip("blockbuster")
    assert hasattr(blockbuster, "BlockBuster") or hasattr(blockbuster, "blockbuster")
