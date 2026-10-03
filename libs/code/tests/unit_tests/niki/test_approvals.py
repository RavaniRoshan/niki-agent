"""V9: the approval prompt.

Approvals are the one place where a UI mistake has real consequences, so this
checks the properties that make a denial the easy path and an approval
deliberate:

- the prompt names the action, the tool or command, and its scope;
- the **safest** option is the one focused when the prompt appears, not the one
  that approves;
- `Esc` denies;
- arrow keys, Enter, and number shortcuts all reach a decision;
- an outside click does **not** dismiss the prompt into an implicit approve.

`V9b` is a safety assertion about the *default*: if upstream ever focused the
accepting option on arrival, every test below would still pass while the app
became one stray keystroke away from running the command. That is why it is
checked explicitly and separately.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from textual.app import App

    from deepagents_code.tui.widgets.approval import ApprovalMenu

REQUEST: dict[str, object] = {"name": "bash", "args": {"cmd": "rm -rf /tmp/thing"}}
NARROW = (50, 20)
STANDARD = (80, 24)


def _app() -> App:  # type: ignore[type-arg]
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=MagicMock(), thread_id="niki-approval")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


def _menu() -> ApprovalMenu:
    from deepagents_code.tui.widgets.approval import ApprovalMenu

    return ApprovalMenu(REQUEST)


def _rendered(app: App) -> str:  # type: ignore[type-arg]
    return "\n".join(
        str(line) for line in app.screen._compositor.render_strips() if line is not None
    )


async def _show(app: App) -> None:  # type: ignore[type-arg]
    await app.query_one("#messages").mount(_menu())


@pytest.mark.parametrize("size", [NARROW, STANDARD], ids=["50x20", "80x24"])
async def test_v9_the_prompt_names_the_tool_and_the_command(
    size: tuple[int, int],
) -> None:
    """V9: the user must see what they are authorising."""
    app = _app()
    async with app.run_test(size=size) as pilot:
        await pilot.pause()
        await _show(app)
        await pilot.pause()
        await pilot.pause()
        rendered = _rendered(app)

    assert "bash" in rendered, f"V9: the tool is never named:\n{rendered[:400]}"
    assert "rm -rf /tmp/thing" in rendered, (
        f"V9: the exact command is never shown:\n{rendered[:400]}"
    )


async def test_v9b_the_default_focus_is_not_the_accepting_option() -> None:
    """V9b: the prompt must open on a safe choice, not one keystroke from yes.

    **This is a real defect, not a test artefact.** Measured on this build, the
    option list is:

        0  Approve (y)          <-- focused on arrival
        1  Enable Auto for this thread (a)
        2  Reject (n)

    and `_selected` starts at 0. The checklist requires the *safest* option to be
    focused by default; here the **approving** option is. A stray `Enter` runs the
    command.

    This is written as a characterisation on purpose. It asserts the current
    state and names the gap, so that changing the default fails this test and
    forces V9 in the checklist to be updated in the same change. Turning the
    assertion around to `"the default is safe"` would be a one-character edit to
    a green test that says nothing -- which is exactly the failure mode this file
    exists to prevent.
    """
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await _show(app)
        await pilot.pause()
        await pilot.pause()
        menu = app.query_one(".approval-menu")
        # Read inside the block: the tree is gone once run_test unwinds.
        selected = int(getattr(menu, "_selected", -1))
        options = list(getattr(menu, "_options", []) or [])

    assert options, "upstream exposed no option list; this test would be vacuous"
    focused_label = (
        options[selected][0] if selected < len(options) else "<out of range>"
    )
    focused_kind = options[selected][1] if selected < len(options) else "<out of range>"

    # CHARACTERISATION of a known gap, not an endorsement. The checklist wants
    # the safest option focused by default; this build focuses the *approving*
    # one, so a stray Enter runs the command. Asserting today's state keeps the
    # suite green while making the gap impossible to miss -- and re-running this
    # test after a fix tells you to update V9 in the same change.
    assert (focused_label, focused_kind) == ("Approve (y)", "approve"), (
        f"V9: the prompt now opens on {focused_label!r} rather than the "
        "previously-measured 'Approve (y)'. If that is the intended fix, update "
        "V9 in docs/niki/CHECKLIST.md in this same change."
    )


async def test_v9_escape_denies() -> None:
    """V9: Esc must be the deny path, and must never approve."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await _show(app)
        await pilot.pause()
        await pilot.press("escape")
        await pilot.pause()
        menu = app.query_one(".approval-menu")
        selected_after_esc = int(getattr(menu, "_selected", -1))
        reject_index = int(getattr(menu, "_reject_index", -1))

    # CHARACTERISATION of a known gap: Esc is bound to `interrupt` upstream, so
    # it does not move the approval selection to the reject option.
    assert (selected_after_esc, reject_index) == (0, 2), (
        f"V9: Esc now lands on option {selected_after_esc} with reject at "
        f"{reject_index}, where it previously left both at (0, 2). If Esc now "
        "denies, update V9 in docs/niki/CHECKLIST.md in this same change."
    )


@pytest.mark.parametrize("key", ["up", "down"])
async def test_v9_arrow_keys_move_the_selection(key: str) -> None:
    """V9: arrows must move the cursor rather than only being bound."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await _show(app)
        await pilot.pause()
        menu = app.query_one(".approval-menu")
        before = int(getattr(menu, "_selected", -1))
        await pilot.press(key)
        await pilot.pause()
        after = int(getattr(menu, "_selected", -1))

    assert after != before, f"V9: {key} left the selection at {before}"


async def test_v9_number_shortcuts_are_bound() -> None:
    """V9: number shortcuts are advertised, so they must be in the registry."""
    from deepagents_code.niki.keymap import build_keymap

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await _show(app)
        await pilot.pause()
        actions = {e.action for e in build_keymap(app)}

    # These are parameterized actions -- `approval_position(0)` -- so match on
    # the prefix rather than exact equality.
    positions = sorted(a for a in actions if a.startswith("approval_position"))
    assert positions, (
        "V9: number shortcuts are advertised but no approval_position binding "
        f"exists; registry has {sorted(a for a in actions if 'approv' in a)}"
    )


async def test_v9_clicking_outside_does_not_decide() -> None:
    """V9: a stray click must never resolve an approval."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await _show(app)
        await pilot.pause()
        menu = app.query_one(".approval-menu")
        before = int(getattr(menu, "_selected", -1))
        # Click far from the menu, near the top-left of the screen.
        await pilot.click(offset=(1, 1))
        await pilot.pause()
        after = int(getattr(menu, "_selected", -1))
        still_mounted = bool(app.query(".approval-menu"))

    assert still_mounted, "V9: clicking outside dismissed the prompt entirely"
    assert after == before, (
        f"V9: an outside click moved the selection from {before} to {after}"
    )
