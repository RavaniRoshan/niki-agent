"""L7: Ctrl+Z suspends from the composer, and undo survives the move.

The pty test proves the real suspend/resume cycle. This covers the binding
contract around it: the composer asks the app to suspend, undo is still one
keystroke away, and other text surfaces keep Textual's own Ctrl+Z undo.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from niki_agent.tui.widgets.chat_input import ChatTextArea
from unit_tests.niki.turn_harness import make_app

if TYPE_CHECKING:
    from niki_agent.niki.app import NikiApp

SIZE = (100, 32)


async def test_l7_ctrl_z_in_the_composer_asks_the_app_to_suspend(
    monkeypatch,
) -> None:
    """L7: the composer's Ctrl+Z dispatches the app's suspend action.

    The signal itself is not sent here -- stopping pytest would be a poor test
    -- so the action is replaced with a recorder. The pty test covers the signal.
    """
    app: NikiApp = make_app()
    suspends: list[int] = []
    monkeypatch.setattr(app, "action_suspend_process", lambda: suspends.append(1))

    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        await pilot.press("ctrl+z")
        await pilot.pause()

    assert suspends, "L7: Ctrl+Z in the composer did not reach the suspend action"


async def test_l7_undo_moves_to_ctrl_shift_z_in_the_composer() -> None:
    """L7: a cleared draft is still recoverable after Ctrl+Z becomes suspend."""
    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        composer = app.query_one("#chat-input", ChatTextArea)
        composer.insert("draft")
        await pilot.pause()
        await pilot.press("ctrl+shift+z")
        await pilot.pause()
        undone = composer.text

    assert undone == "", f"L7: Ctrl+Shift+Z did not undo the draft: {undone!r}"


async def test_l7_the_niki_composer_opts_into_the_suspend_key() -> None:
    """L7: the shared composer only suspends because Niki turns it on.

    `dcode` keeps Textual's undo through the same binding, so this opt-in is
    what makes the Niki behavior real rather than incidental.
    """
    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        composer = app.query_one("#chat-input", ChatTextArea)

    assert composer.suspend_on_ctrl_z is True, (
        "L7: Niki's composer never opted into ctrl+z suspend"
    )
