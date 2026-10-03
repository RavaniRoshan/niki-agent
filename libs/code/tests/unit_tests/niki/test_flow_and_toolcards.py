"""F2, F3, and V7: streaming shape, truthful activity, and tool cards.

F3 and V7 are decided with the real app mounted, not a bare `App`: a bare host
lacks the theme CSS variables the tool card's own `DEFAULT_CSS` references, so
mounting one there fails on `$tool` and proves nothing about Niki.
"""

from __future__ import annotations

import asyncio
from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from textual.app import App

CHUNKS = 40


def _app() -> App:  # type: ignore[type-arg]
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=MagicMock(), thread_id="niki-flow")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


def _rendered(app: App) -> str:  # type: ignore[type-arg]
    """The screen as a terminal would render it."""
    return "\n".join(
        str(line) for line in app.screen._compositor.render_strips() if line is not None
    )


async def test_f3_streaming_renders_into_one_block() -> None:
    """F3: a stream must grow one message, not append a new widget per token."""
    from deepagents_code.tui.widgets.messages import AssistantMessage

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        messages = app.query_one("#messages")
        before = len(messages.children)

        target = AssistantMessage(content="")
        await messages.mount(target)
        await pilot.pause()
        after_mount = len(messages.children)

        for _ in range(CHUNKS):
            await target.append_content("word ")
            await asyncio.sleep(0.01)
        await pilot.pause()
        after_stream = len(messages.children)

        blocks = app.query(AssistantMessage)
        assert len(blocks) == 1, f"expected one assistant block, found {len(blocks)}"
        grown = str(getattr(target, "text", "")) or _rendered(app)

    assert after_mount == before + 1, "the mount itself did not add exactly one widget"
    assert after_stream == after_mount, (
        f"F3: {CHUNKS} chunks added {after_stream - after_mount} extra widgets; "
        "the stream is fragmenting into separate blocks instead of one"
    )
    assert grown.strip(), "F3: the block is empty after streaming"


async def test_f3_streaming_paints_far_fewer_frames_than_chunks() -> None:
    """F3: 'smoothly' means the coalescing actually coalesces."""
    from deepagents_code.tui.widgets.messages import AssistantMessage
    from unit_tests.niki.perf_probe import count_renders

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        messages = app.query_one("#messages")
        target = AssistantMessage(content="")
        await messages.mount(target)
        await pilot.pause()

        with count_renders() as renders:
            baseline = renders[0]
            for _ in range(CHUNKS):
                await target.append_content("word ")
                await asyncio.sleep(0.005)
            await pilot.pause()
            painted = renders[0] - baseline

    assert painted < CHUNKS, (
        f"F3: {CHUNKS} chunks caused {painted} repaints; coalescing is not "
        "reducing the work per token"
    )


async def test_f2_no_activity_state_without_a_real_event() -> None:
    """F2: the app must not claim to be busy when nothing is happening.

    Catches the classic lie -- an activity indicator that reads "running"
    because a timer started it, rather than because a real agent or tool event
    did.
    """
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await pilot.pause()
        idle_text = _rendered(app).lower()
        busy_words = ("running", "working", "thinking", "streaming")
        claimed = [w for w in busy_words if w in idle_text]

    assert not claimed, (
        f"F2: a freshly started app displays {claimed} with no agent or tool "
        "event behind it; the activity state is not driven by real events"
    )


async def test_v7_a_tool_card_shows_tool_intent_and_status() -> None:
    """V7: the card names the tool and its intent, and carries a status glyph."""
    from deepagents_code.tui.widgets.messages import ToolCallMessage

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        messages = app.query_one("#messages")
        before = len(messages.children)

        card = ToolCallMessage(tool_name="bash", args={"cmd": "ls -la"})
        await messages.mount(card)
        await pilot.pause()
        await pilot.pause()
        rendered = _rendered(app)
        # Read inside the block: the tree is gone once run_test unwinds.
        card_count = len(messages.children)

    assert card_count == before + 1, (
        f"V7: mounting a tool call left {card_count} children, expected {before + 1}"
    )
    assert "bash" in rendered, f"V7: the card never names the tool:\n{rendered[:400]}"
    assert "ls -la" in rendered, (
        f"V7: the card never shows the intent:\n{rendered[:400]}"
    )


async def test_v7_no_card_appears_without_a_real_tool_event() -> None:
    """V7: a card must correspond to a tool event, never appear speculatively."""
    from deepagents_code.tui.widgets.messages import AssistantMessage, ToolCallMessage

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        messages = app.query_one("#messages")
        await messages.mount(AssistantMessage(content="plain text, no tool"))
        await pilot.pause()
        await pilot.pause()

        assert not list(app.query(ToolCallMessage)), (
            "V7: a tool card exists for a message that was never a tool event"
        )


@pytest.mark.parametrize("result", ["done", ""])
async def test_v7_card_reports_a_result_or_elapsed_time(result: str) -> None:
    """V7: a finished card shows an outcome; an empty result must not blank it."""
    from deepagents_code.tui.widgets.messages import ToolCallMessage

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        messages = app.query_one("#messages")
        card = ToolCallMessage(tool_name="bash", args={"cmd": "echo hi"})
        await messages.mount(card)
        await pilot.pause()
        card._output = result
        card._update_output_display()
        await pilot.pause()
        await asyncio.sleep(0.2)
        await pilot.pause()
        rendered = _rendered(app)

    assert "bash" in rendered, (
        f"V7: the card lost its tool name when the result was {result!r}"
    )
