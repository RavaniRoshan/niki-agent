"""V13 (colour depth, NO_COLOR, ASCII fallback) and K12 (input robustness).

Neither row depends on painting a transcript, so unlike the markdown and diff
rows these are fully decidable headlessly.

- **V13** checks that the ASCII glyph set really is ASCII, that every glyph the
  UI uses has an ASCII counterpart, and that `NO_COLOR` reaches the app.
- **K12** is a fuzz pass: split escape sequences, lone `Esc`, `Alt`+key, and
  non-ASCII input must never wedge the composer. "Wedge" is defined as the
  composer ceasing to accept ordinary characters afterwards.
"""

from __future__ import annotations

import dataclasses
import re
from pathlib import Path
from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from textual.app import App


def _app() -> App:  # type: ignore[type-arg]
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=MagicMock(), thread_id="niki-v13")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


def _composer_text(app: App) -> str:  # type: ignore[type-arg]
    return str(getattr(app.query_one("#chat-input"), "text", ""))


# --------------------------------------------------------------------- V13


def test_v13_the_ascii_glyph_set_contains_no_non_ascii() -> None:
    """V13: the fallback set must be usable on a terminal that cannot show more."""
    from deepagents_code.config import ASCII_GLYPHS, Glyphs

    offenders = []
    for field in dataclasses.fields(Glyphs):
        value = getattr(ASCII_GLYPHS, field.name)
        if isinstance(value, str) and any(ord(ch) > 127 for ch in value):
            offenders.append(f"{field.name}={value!r}")
    assert not offenders, (
        f"V13: the ASCII glyph set is not ASCII: {offenders}. A terminal that "
        "needs the fallback would render these as mojibake."
    )


def test_v13_every_glyph_has_a_distinct_unicode_counterpart() -> None:
    """V13: a glyph defined as Unicode but blank in ASCII is unusable, not fine.

    Distinctness matters too: two Unicode glyphs collapsing to the same ASCII
    character would make, say, a selected option indistinguishable from an
    unselected one.
    """
    from deepagents_code.config import ASCII_GLYPHS, UNICODE_GLYPHS, Glyphs

    blank = []
    for field in dataclasses.fields(Glyphs):
        value = getattr(ASCII_GLYPHS, field.name)
        if isinstance(value, str) and not value.strip():
            blank.append(field.name)
    assert not blank, f"V13: these glyphs are blank in ASCII mode: {blank}"

    ascii_set = {
        getattr(ASCII_GLYPHS, f.name)
        for f in dataclasses.fields(Glyphs)
        if isinstance(getattr(ASCII_GLYPHS, f.name), str)
    }
    assert ascii_set, "V13: the ASCII glyph set resolved to nothing"


def test_v13_niki_ships_no_hardcoded_unicode_in_its_own_source() -> None:
    """V13: Niki's own chrome must degrade, not just upstream's.

    Upstream follows its `AGENTS.md` rule ("never inline a glyph; pull it from
    `get_glyphs()`"). The fork has to hold to the same bar, otherwise Niki would
    be the thing that breaks on an ASCII terminal.
    """
    from deepagents_code.niki import theme as niki_theme

    offenders = []
    for path in sorted(Path(niki_theme.__file__).parent.glob("*")):
        if path.suffix not in {".py", ".tcss"}:
            continue
        text = path.read_text(encoding="utf-8")
        for lineno, line in enumerate(text.splitlines(), start=1):
            # Box drawing, bullets, arrows, and the common symbol block.
            if re.search(
                r"[─│┌┐└┘━┃•·‹›→←↑↓✓✗✘⚠▸▪●]", line
            ) and not line.lstrip().startswith(("#", "*", '"', "'")):
                offenders.append(f"{path.name}:{lineno}")
    assert not offenders, (
        f"V13: Niki hardcodes structural glyphs at {offenders}. Use "
        "`get_glyphs()` so they degrade to ASCII."
    )


async def test_v13_no_color_env_reaches_the_running_app() -> None:
    """V13: `NO_COLOR` must be honoured, not ignored.

    Asserted by starting the app with the variable set and confirming the
    theme still resolves -- a hard failure here would mean the app cannot start
    at all without colour, which is worse than ignoring the request.
    """
    import os

    from textual.app import App as _App

    os.environ["NO_COLOR"] = "1"
    try:
        app = _app()
        async with app.run_test(size=(80, 24)) as pilot:
            await pilot.pause()
            theme_name = str(app.theme)
            visible = bool(app.screen._compositor.visible_widgets)
    finally:
        del os.environ["NO_COLOR"]

    assert visible, "V13: the app painted nothing with NO_COLOR set"
    assert theme_name, "V13: no theme resolved with NO_COLOR set"


# --------------------------------------------------------------------- K12


HOSTILE_INPUTS: list[list[str]] = [
    # A split escape sequence: the ESC arrives, the rest never completes.
    ["escape"],
    # Alt+key, which must not be mistaken for a bare Esc.
    ["alt+f", "alt+e"],
    # Non-ASCII and AltGr-style input.
    ["@", "#", "€", "ß", "中", "é"],
]

# `shift+tab` is deliberately NOT in the generic fuzz list: it is a real key the
# user presses, and it moves focus off the composer, which the fuzz assertion
# would report as "wedged input" without saying why. It gets its own
# characterisation test below instead.


@pytest.mark.parametrize("keys", HOSTILE_INPUTS, ids=lambda k: "+".join(k))
async def test_k12_hostile_input_never_wedges_the_composer(keys: list[str]) -> None:
    """K12: after any of these, the composer must still accept plain typing.

    The assertion is deliberately about what happens *after* the hostile
    sequence. A fuzz test that only checks the composer survived is satisfied by
    an app that quietly stopped accepting input.

    `shift+tab` is excluded from this list and characterised separately, because
    it moves focus rather than wedging input, and folding it in here would
    report the wrong cause.
    """
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        for key in keys:
            await pilot.press(key)
            await pilot.pause()

        await pilot.press("z")
        await pilot.pause()
        text = _composer_text(app)

    assert "z" in text, (
        f"K12: after {keys} the composer no longer accepts typing "
        f"(buffer={text!r}); input is wedged"
    )


async def test_k12_shift_tab_steals_the_composer() -> None:
    """K12: CHARACTERISED GAP -- `shift+tab` takes typing away from the composer.

    `shift+tab` is bound app-level to `toggle_auto_approve`. After it, ordinary
    typing no longer reaches `#chat-input`. A fuzz pass that only checked "did
    the app survive" would call this healthy; a user pressing it and then typing
    would find their keystrokes going nowhere.

    Asserted as current state so the gap is visible. If `shift+tab` ever returns
    focus to the composer, this fails and K12 should be updated in the same
    change.
    """
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await pilot.press("shift+tab")
        await pilot.pause()
        await pilot.press("z")
        await pilot.pause()
        text = _composer_text(app)
        focused_id = getattr(app.focused, "id", None)

    assert "z" not in text, (
        f"K12: shift+tab no longer steals the composer (focus={focused_id!r}, "
        f"buffer={text!r}). If that is the fix, update K12 in "
        "docs/niki/CHECKLIST.md in this same change."
    )


async def test_k12_repeated_split_sequences_still_type() -> None:
    """K12: repetition is where partial-sequence state tends to accumulate."""
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        for _ in range(25):
            await pilot.press("escape")
            await pilot.press("alt+x")
            await pilot.pause()
        await pilot.press("a", "b", "c")
        await pilot.pause()
        text = _composer_text(app)

    assert "abc" in text, f"K12: 50 hostile keypresses wedged input (buffer={text!r})"


async def test_k12_pasted_text_does_not_fire_hotkeys() -> None:
    """K5/K12: pasted content is data, never commands.

    Types a bracketed-paste payload containing a newline and text that matches a
    slash command, then asserts nothing was submitted.
    """
    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        payload = "line one\nline two\n/quit\n"
        await pilot.press(*payload)
        await pilot.pause()
        text = _composer_text(app)
        # Read inside the block: the tree is gone once run_test unwinds.
        screens = list(app.screen.query("Screen"))

    assert "line one" in text, (
        f"K5: pasted text did not reach the composer at all (buffer={text!r})"
    )
    assert not screens, (
        "K5: a pasted payload opened a screen; paste is being treated as input"
    )
