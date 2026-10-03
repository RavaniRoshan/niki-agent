"""M3 (hover is throttled, no redraw storm) and V6 (the footer tells the truth).

Both are decidable headlessly, unlike the on-screen rendering rows:

- **M3** posts real `MouseMove` events and counts repaints through the
  compositor, so "hover does not cause a redraw storm" is a measurement rather
  than an impression.
- **V6** reads the status bar's own rendered text and checks it against facts
  that can be established independently — the working directory, the git branch,
  and the permission mode. Truthfulness means the footer agrees with reality, not
  that it renders.
"""

from __future__ import annotations

import subprocess
from pathlib import Path
from typing import TYPE_CHECKING

import pytest

from unit_tests.niki.perf_probe import count_renders

if TYPE_CHECKING:
    from textual.app import App
    from textual.pilot import Pilot

HOVER_MOVES = 120
SIZES = [(50, 16), (80, 24), (120, 38), (160, 45)]


def _app() -> App:  # type: ignore[type-arg]
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp

    app = NikiApp(agent=MagicMock(), thread_id="niki-footer")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


def _status_text(app: App) -> str:  # type: ignore[type-arg]
    """The footer's rendered text, read from the bottom row of the screen.

    A widget has no compositor of its own -- the screen composites the tree --
    so the honest way to read what the footer *displays* is to take the screen's
    last non-empty row. Reading the widget's internal renderables would test the
    data rather than what a user sees.
    """
    strips = [strip for strip in app.screen._compositor.render_strips() if strip]
    for strip in reversed(strips):
        row = strip.text.rstrip()
        if row.strip():
            return row
    return ""


async def _hover(app: App, pilot: Pilot[None], moves: int) -> int:
    """Sweep the pointer across the composer and return the repaint count."""
    from textual.events import MouseMove

    composer = app.query_one("#chat-input")
    top = composer.region.y + 1
    with count_renders() as renders:
        baseline = renders[0]
        for index in range(moves):
            event = MouseMove(
                composer,
                x=float(index % 40),
                y=float(top),
                delta_x=1,
                delta_y=0,
                button=0,
                shift=False,
                meta=False,
                ctrl=False,
            )
            app.screen.post_message(event)
            if index % 10 == 0:
                # Let the loop breathe every tenth move; posting all of them
                # without yielding would measure the queue, not the repaints.
                await pilot.pause()
        await pilot.pause()
        return renders[0] - baseline


async def test_m3_hovering_does_not_cause_a_redraw_storm() -> None:
    """M3: sweeping the pointer must not repaint per event."""
    app = _app()
    async with app.run_test(size=(120, 38)) as pilot:
        await pilot.pause()
        repaints = await _hover(app, pilot, HOVER_MOVES)

    # A repaint per hover event would be 120. Coalescing should keep this far
    # below it; the ceiling is deliberately loose because this counts frames,
    # not work, and the real complaint is a visible flicker storm.
    assert repaints < HOVER_MOVES, (
        f"M3: {HOVER_MOVES} hover events caused {repaints} repaints -- one per "
        "event, which is the redraw storm this row forbids"
    )


async def test_m3_hovering_leaves_the_ui_painting() -> None:
    """M3: the guard above must not pass by hovering into a broken state."""
    app = _app()
    async with app.run_test(size=(120, 38)) as pilot:
        await pilot.pause()
        await _hover(app, pilot, 20)
        await pilot.pause()
        painted = bool(app.screen._compositor.visible_widgets)

    assert painted, "M3: the screen went blank after hovering"


# ------------------------------------------------------------------------ V6


def _git_branch(cwd: str) -> str:
    """The current git branch, read independently of the app."""
    try:
        out = subprocess.run(
            ["git", "-C", cwd, "rev-parse", "--abbrev-ref", "HEAD"],
            capture_output=True,
            text=True,
            check=False,
            timeout=15,
        )
        return out.stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return ""


@pytest.mark.parametrize("size", SIZES, ids=[f"{w}x{h}" for w, h in SIZES])
async def test_v6_the_footer_reports_real_facts(size: tuple[int, int]) -> None:
    """V6: whatever the footer claims must be independently checkable.

    Asserted against facts established outside the app: the working directory,
    the git branch, and the permission mode. A footer that shows a stale cwd or
    the wrong branch is worse than no footer.
    """
    app = _app()
    async with app.run_test(size=size) as pilot:
        await pilot.pause()
        await pilot.pause()
        text = _status_text(app)
        mode = str(getattr(app, "approval_mode", "") or "")
        cwd = str(Path.cwd())

    assert text.strip(), f"V6: the status bar rendered nothing at {size[0]}x{size[1]}"
    assert "manual" in text.lower(), (
        f"V6: the permission mode is not shown; footer reads {text!r} "
        f"(app mode: {mode!r})"
    )

    branch = _git_branch(cwd)
    if branch:
        assert branch in text, (
            f"V6: the git branch {branch!r} is not shown in the footer: {text!r}"
        )

    # V6 requires the footer to *collapse by width*, not wrap. So the cwd is
    # required only where there is room for it; below that the footer is
    # allowed -- expected, in fact -- to drop segments rather than overflow.
    if size[0] >= 100:
        short_cwd = Path(cwd).name
        assert short_cwd in text or "~" in text, (
            f"V6: at {size[0]} columns there is room for the working directory, "
            f"but the footer reads {text!r}"
        )
    else:
        assert len(text) <= size[0], (
            f"V6: the footer is {len(text)} characters inside {size[0]} columns; "
            "it must drop segments rather than overflow"
        )


async def test_v6_the_footer_never_claims_more_than_it_knows() -> None:
    """V6: a footer must not invent numbers.

    With a mock agent there is no model, no usage, and no cost. If the footer
    reports a token count or a dollar figure here, it is making it up.
    """
    app = _app()
    async with app.run_test(size=(120, 38)) as pilot:
        await pilot.pause()
        await pilot.pause()
        text = _status_text(app).lower()

    for invented in ("$0.", "0.00", "tokens:", "context:"):
        assert invented not in text, (
            f"V6: the footer reports {invented!r} with no agent behind it: "
            f"{text!r}. A number the app cannot know is worse than no number."
        )


@pytest.mark.parametrize("size", SIZES, ids=[f"{w}x{h}" for w, h in SIZES])
async def test_v6_the_footer_fits_its_width(size: tuple[int, int]) -> None:
    """V6: the footer is one line and never wider than the terminal.

    Overflow is what makes a status bar wrap into the transcript.
    """
    app = _app()
    async with app.run_test(size=size) as pilot:
        await pilot.pause()
        await pilot.pause()
        try:
            bar = app.query_one("#status-bar")
            height = bar.region.height
            width = bar.region.width
        except Exception:  # noqa: BLE001
            pytest.fail(f"V6: no status bar at {size[0]}x{size[1]}")

    assert height <= 1, f"V6: the status bar occupies {height} rows at {size}"
    assert width <= size[0], (
        f"V6: the status bar is {width} wide inside a {size[0]} wide terminal"
    )
