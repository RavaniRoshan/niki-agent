"""V4: the transcript distinguishes the user's turns from the assistant's.

The design direction is specific:

- assistant text is **unboxed on the base background**;
- user turns are marked by a **thin gutter** or a **subtle raised background**;
- hierarchy comes from weight, dimness, and whitespace, with **at most one
  border level**.

Measured on this build, upstream already satisfies that and Niki's theme carries
it: the user's turn has `border-left: wide` in `$primary` plus a 15 %-alpha
background, while the assistant's has neither. Because the gutter is drawn in
`$primary`, rebranding the theme recolours it with no extra work -- which is the
property this file exists to pin.

Asserted at the **style** level rather than by reading pixels. Pixel assertions
for this row are not decidable headlessly (see `test_markdown.py` for the
measured reason), and a style assertion is the more precise claim anyway: it
states the rule, not a picture of it.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from textual.app import App
    from textual.pilot import Pilot

    from deepagents_code.tui.widgets.messages import AssistantMessage, UserMessage


def _app() -> App:  # type: ignore[type-arg]
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=MagicMock(), thread_id="niki-v4")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


async def _mount_pair(
    app: App, pilot: Pilot[None]
) -> tuple[UserMessage, AssistantMessage]:
    """Mount one user turn and one assistant turn side by side."""
    from deepagents_code.tui.widgets.messages import AssistantMessage, UserMessage

    await app.query_one("#messages").mount(UserMessage("what is 2+2"))
    await app.query_one("#messages").mount(AssistantMessage("4"))
    await pilot.pause()
    user = app.query_one(UserMessage)
    assistant = app.query_one(AssistantMessage)
    return user, assistant


async def test_v4_the_assistant_turn_is_unboxed() -> None:
    """V4: assistant text sits on the base background with no border."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        _, assistant = await _mount_pair(app, pilot)
        border = assistant.styles.border
        background = assistant.styles.background

    assert border.left[0] == "", (
        f"V4: the assistant turn has a left border {border.left}"
    )
    assert border.top[0] == "", f"V4: the assistant turn has a top border {border.top}"
    assert background.a == 0, (
        f"V4: the assistant turn has a raised background (alpha {background.a}); "
        "assistant text must sit unboxed on the base"
    )


async def test_v4_the_user_turn_is_marked_by_a_gutter() -> None:
    """V4: a user turn must be findable at a glance without reading it."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        user, _ = await _mount_pair(app, pilot)
        border = user.styles.border
        background = user.styles.background
        raised = background.a > 0

    assert border.left[0] == "wide", (
        f"V4: the user turn has no left gutter (border-left={border.left}); the "
        "transcript gives the eye nothing to find 'what did I say' by"
    )
    assert border.top[0] == "", (
        "V4: the user turn has a top border; only one border level is permitted"
    )
    assert raised, (
        "V4: the user turn has neither a gutter's weight nor a raised background"
    )


async def test_v4_the_gutter_is_drawn_in_the_niki_accent() -> None:
    """V4: the gutter must follow the theme, not a hardcoded colour.

    This is the property that makes the rebrand hold: the gutter is drawn in
    `$primary`, so swapping the theme recolours it with no extra work. A literal
    here would silently keep the old palette's colour forever.
    """
    from deepagents_code.niki.theme import NIKI_DARK

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        user, _ = await _mount_pair(app, pilot)
        gutter = user.styles.border.left[1].hex.lower()

    assert gutter.endswith(NIKI_DARK["primary"][1:].lower()), (
        f"V4: the gutter is {gutter}, not Niki's primary "
        f"{NIKI_DARK['primary']}. A literal colour here would survive a "
        "theme change and leave the old palette behind."
    )


async def test_v4_exactly_one_border_level_is_used() -> None:
    """V4: at most one border level across the transcript.

    Boxes everywhere is what makes a terminal feel like a log file. The only
    border permitted in the transcript is the user gutter.
    """
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        user, assistant = await _mount_pair(app, pilot)
        user_bordered = [
            edge
            for edge in (
                user.styles.border.left,
                user.styles.border.right,
                user.styles.border.top,
                user.styles.border.bottom,
            )
            if edge[0]
        ]
        assistant_bordered = [
            edge
            for edge in (
                assistant.styles.border.left,
                assistant.styles.border.right,
                assistant.styles.border.top,
                assistant.styles.border.bottom,
            )
            if edge[0]
        ]

    assert len(user_bordered) == 1, (
        f"V4: the user turn uses {len(user_bordered)} border edges; the design "
        "permits one level, not a box"
    )
    assert not assistant_bordered, (
        f"V4: the assistant turn is boxed on {len(assistant_bordered)} edges"
    )
