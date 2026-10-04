"""Niki's motion language: glyphs, cadence, reduced motion, and ASCII fallback.

Two properties matter more than the specific glyphs:

1. **Motion is never the only signal.** Under reduced motion or an ASCII
   terminal, the indicator still has to be *readable*. A spinner that disappears
   entirely is worse than one that stands still.
2. **Motion never costs anything when idle.** The S4 row holds the app to zero
   repaints over 5 idle seconds; an animation must not be the thing that breaks
   it. `test_reduced_motion_does_not_add_idle_repaint_cost` checks the two do not
   fight each other.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from deepagents_code.niki.motion import (
    CADENCE,
    CADENCE_ACTIVE,
    CADENCE_CALM,
    NIKI_SPINNER_FRAMES,
    NIKI_SPINNER_FRAMES_ASCII,
    cadence_for,
    charset_is_ascii,
    frame_at_tick,
    reduced_motion,
    spinner_frames_for_this_terminal,
)

if TYPE_CHECKING:
    from textual.app import App

#: The two frame sets are deliberately different from both upstream's braille
#: and the asterisk family used elsewhere. The brief forbids reproducing another
#: tool's glyph set, so this asserts we did not.
UPSTREAM_BRAILLE = ("⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏")
ASTERISK_FAMILY = {"·", "✢", "✳", "✶", "✻", "✽"}


def test_the_unicode_motif_is_distinct_from_other_tools() -> None:
    """The glyph vocabulary must be Niki's own, not a copy."""
    frames = set(NIKI_SPINNER_FRAMES)

    assert frames.isdisjoint(UPSTREAM_BRAILLE), "Niki reuses upstream's braille spinner"
    assert frames.isdisjoint(ASTERISK_FAMILY), (
        f"Niki reuses the asterisk family used by other agents: "
        f"{frames & ASTERISK_FAMILY}"
    )
    assert len(frames) == len(NIKI_SPINNER_FRAMES), "duplicate frames waste a tick"


def test_the_motif_pings_and_pongs_rather_than_wrapping() -> None:
    """The sweep must reverse at each end; a wrap reads as a glitch."""
    sequence = [frame_at_tick(NIKI_SPINNER_FRAMES, tick)[1] for tick in range(9)]
    expected = ["◐", "◓", "◑", "◒", "◑", "◓", "◐", "◓", "◑"]

    assert sequence == expected, f"unexpected sweep: {' '.join(sequence)}"


def test_the_ascii_motif_is_pure_ascii() -> None:
    """The fallback must be usable on a terminal that cannot show more."""
    for frame in NIKI_SPINNER_FRAMES_ASCII:
        assert all(ord(ch) < 128 for ch in frame), f"{frame!r} is not ASCII"


def test_frames_are_single_width_codepoints() -> None:
    """An ambiguous-width glyph makes a spinner jitter as it advances."""
    for frame in (*NIKI_SPINNER_FRAMES, *NIKI_SPINNER_FRAMES_ASCII):
        assert len(frame) == 1, (
            f"{frame!r} is more than one codepoint; a spinner built from "
            "multi-codepoint frames jitters on terminals that guess width"
        )


def test_frame_at_tick_is_stateless() -> None:
    """The same tick must always give the same frame, so pausing cannot desync.

    The animation timer is paused and resumed constantly -- during a resize, a
    modal, a permission prompt. If the frame were derived from a mutable
    counter, resuming would jump the animation to a frame that does not match
    what is on screen.
    """
    for tick in (0, 3, 7, 11):
        first = frame_at_tick(NIKI_SPINNER_FRAMES, tick)
        second = frame_at_tick(NIKI_SPINNER_FRAMES, tick)
        assert first == second


def test_frame_at_tick_handles_an_empty_set() -> None:
    """An empty frame set must not divide by zero or raise."""
    assert frame_at_tick((), 3) == (0, "")


@pytest.mark.parametrize(
    ("elapsed", "active", "expected"),
    [
        (0.0, False, CADENCE),
        (5.0, False, CADENCE),
        (5.0, True, CADENCE_ACTIVE),
        (30.0, False, CADENCE_CALM),
    ],
)
def test_cadence_responds_to_elapsed_time_and_activity(
    elapsed: float, active: bool, expected: float
) -> None:
    """A long turn should calm down; an active one should speed up."""
    assert cadence_for(elapsed, active=active) == expected


@pytest.mark.parametrize("value", ["1", "true", "yes", "on", "TRUE", " On "])
def test_reduced_motion_is_honoured(
    value: str, monkeypatch: pytest.MonkeyPatch
) -> None:
    """V10: a user who needs stillness must be able to ask for it."""
    monkeypatch.delenv("NO_MOTION", raising=False)
    monkeypatch.setenv("NIKI_REDUCED_MOTION", value)
    assert reduced_motion() is True


def test_no_motion_is_honoured_like_no_color(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The well-known convention, accepted for the same reason `NO_COLOR` is."""
    monkeypatch.delenv("NIKI_REDUCED_MOTION", raising=False)
    monkeypatch.setenv("NO_MOTION", "1")
    assert reduced_motion() is True


def test_motion_is_on_by_default(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("NIKI_REDUCED_MOTION", raising=False)
    monkeypatch.delenv("NO_MOTION", raising=False)
    assert reduced_motion() is False


def test_reduced_motion_yields_one_static_frame(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Motion reduces; the indicator must not disappear with it."""
    monkeypatch.setenv("NIKI_REDUCED_MOTION", "1")
    monkeypatch.delenv("UI_CHARSET_MODE", raising=False)

    frames = spinner_frames_for_this_terminal()

    assert len(frames) == 1, (
        "reduced motion must collapse to a single frame, not keep animating"
    )


def test_ascii_charset_yields_ascii_frames(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A forced ASCII charset must never receive the quadrant motif."""
    monkeypatch.delenv("NIKI_REDUCED_MOTION", raising=False)
    monkeypatch.setenv("UI_CHARSET_MODE", "ascii")

    frames = spinner_frames_for_this_terminal()

    assert charset_is_ascii() is True
    assert all(ord(f) < 128 for f in frames)
    assert frames[0] == NIKI_SPINNER_FRAMES_ASCII[0]


def test_reduced_motion_and_ascii_compose(monkeypatch: pytest.MonkeyPatch) -> None:
    """Both switches at once must still produce a usable frame."""
    monkeypatch.setenv("NIKI_REDUCED_MOTION", "1")
    monkeypatch.setenv("UI_CHARSET_MODE", "ascii")

    frames = spinner_frames_for_this_terminal()

    assert frames == ("*",), f"expected one ASCII static frame, got {frames}"


async def test_reduced_motion_does_not_add_idle_repaint_cost(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """S4 and V10 must not fight: reduced motion is not an excuse for motion."""
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp
    from unit_tests.niki.perf_probe import count_renders

    monkeypatch.setenv("NIKI_REDUCED_MOTION", "1")
    app = NikiApp(agent=MagicMock(), thread_id="niki-motion")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]

    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await pilot.pause()
        with count_renders() as renders:
            await pilot.pause()
            idle = renders[0]

    assert idle == 0, f"{idle} repaints while idle with reduced motion on"


def test_the_motion_module_hardcodes_no_color() -> None:
    """V1: the motion language may be about colour, but may not contain any."""
    import re
    from pathlib import Path

    import deepagents_code.niki.motion as module

    text = Path(module.__file__).read_text(encoding="utf-8")
    # Match the same rule as `test_lint_rules.py`: hex literals, not every `#`.
    assert not re.search(r"#[0-9A-Fa-f]{6}\b", text), (
        "the motion module must not define colours; every colour comes from the "
        "theme module"
    )


def test_niki_glyphs_are_only_installed_for_niki() -> None:
    """The glyph rebind must not happen at package import.

    If it did, importing `deepagents_code.niki` would change `dcode`'s glyphs in
    the same process, and upstream's tests would see Niki's frames.
    """
    import deepagents_code.config as config_module

    # Resolving Niki's glyphs must not mutate the shared cache by itself.
    before = getattr(config_module, "_glyphs_cache", None)
    spinner_frames_for_this_terminal()
    assert getattr(config_module, "_glyphs_cache", None) is before
