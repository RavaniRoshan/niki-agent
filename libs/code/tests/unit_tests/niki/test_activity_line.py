"""V10: the activity line.

Covers what is decidable headlessly:

- the spinner advances and has both Unicode and ASCII frame sets;
- nothing claims to be busy while idle (F2 covers the *text*; this covers the
  *animation* -- a spinner that ticks forever is the same lie in a different
  costume, and S4's zero-idle-repaint probe already implies it stops);
- reduced motion has a static equivalent.

That last one is a **gap, not a pass**: there is no reduced-motion preference
anywhere in the package (no `REDUCED_MOTION` env var, no config key), so a user
who gets motion sick has no way to ask for stillness. It is characterised below
rather than quietly skipped.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from textual.app import App
    from textual.pilot import Pilot


def _app() -> App:  # type: ignore[type-arg]
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=MagicMock(), thread_id="niki-v10")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


def test_v10_the_spinner_advances() -> None:
    """V10: the activity indicator must actually animate while working."""
    from deepagents_code.tui.widgets.loading import Spinner

    spinner = Spinner()
    frames = [spinner.current_frame()]
    for _ in range(len(spinner.frames) if hasattr(spinner, "frames") else 6):
        spinner.next_frame()
        frames.append(spinner.current_frame())

    assert len(set(frames)) > 1, (
        "V10: the spinner never changes frame; the activity line would look "
        "frozen while the agent works"
    )


def test_v10_the_spinner_follows_the_charset() -> None:
    r"""V10: the activity indicator must degrade, not stay braille-only.

    Asserted on the charset switch rather than on the default. The default is
    braille on a Unicode terminal, which is correct -- an earlier version of this
    test asserted the *default* frames were ASCII and failed for the right
    reason: braille is the right choice when the terminal can show it.

    What matters is that the frames come from the charset-aware glyph set, so an
    ASCII terminal gets `(-) (\\) (|) (/)`.
    """
    from deepagents_code.config import get_glyphs
    from deepagents_code.tui.widgets.loading import Spinner

    glyph_frames = get_glyphs().spinner_frames
    spinner = Spinner()

    produced = set()
    for _ in range(8):
        produced.add(spinner.current_frame())
        spinner.next_frame()

    known = {str(f) for f in glyph_frames}
    assert produced <= known or not known, (
        f"V10: the spinner emitted {sorted(produced)} which are not drawn from "
        f"get_glyphs().spinner_frames ({sorted(known)}); it will not degrade on "
        "an ASCII terminal"
    )


async def test_v10_nothing_animates_while_idle() -> None:
    """V10: the activity line must stop at idle.

    Uses the compositor repaint counter rather than inspecting timer state --
    what the user sees is whether anything moves, so that is what is measured.
    """
    from unit_tests.niki.perf_probe import count_renders

    app = _app()
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await pilot.pause()
        with count_renders() as renders:
            baseline = renders[0]
            await pilot.pause()
            idle_paints = renders[0] - baseline

    assert idle_paints == 0, (
        f"V10: {idle_paints} repaints while idle with no work in flight; the "
        "activity line is animating for nobody"
    )


def test_v10_reduced_motion_has_no_static_equivalent() -> None:
    """V10: CHARACTERISED GAP -- reduced motion is not implemented.

    The checklist asks for "a static equivalent for reduced motion". There is no
    reduced-motion preference in the package at all: no `REDUCED_MOTION`
    environment variable, no `[ui]` config key, nothing for a user to set.

    Asserted as current state so the gap is visible. If a reduced-motion
    preference is added, this test fails and V10 can be marked complete in the
    same change.
    """
    import deepagents_code
    from deepagents_code import _env_vars

    names = [
        n for n in dir(_env_vars) if "REDUCED" in n.upper() or "MOTION" in n.upper()
    ]
    assert not names, (
        f"V10: reduced-motion settings now exist ({names}). Implement the static "
        "equivalent, then mark V10 complete in docs/niki/CHECKLIST.md in this "
        "same change."
    )
    assert deepagents_code.__name__ == "deepagents_code"
