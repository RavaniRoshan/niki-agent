"""Niki's motion language: glyphs, cadence, and the reduced-motion switch.

Three things live here, and all three are Niki's own:

**The activity glyph.** A rotating quadrant sweep — `◐◓◑◒` — ping-ponging so the
motion reverses at each end rather than snapping back. Upstream ships braille
(`⠋⠙⠹…`) and Claude Code ships an asterisk family (`·✢✳✶✻✽`); this is
deliberately neither. The quadrant glyph reads as a continuous rotation at any
size, and it is one codepoint wide everywhere, which matters because a glyph
with an ambiguous width makes a spinner jitter -- the reason Ghostty users see a
different spinner set in Claude Code.

**The cadence.** 120 ms per frame, 100 ms when the work is known to be waiting
on something (a tool, a rate limit) and a slower, calmer 200 ms once a turn has
run long enough that urgency reads as noise.

**Reduced motion.** Honours `NIKI_REDUCED_MOTION` and `NO_MOTION`. When set,
every animated element degrades to a single static glyph and the timers stop
altogether. This is not cosmetic: continuous motion is a documented accessibility
problem, and it also costs CPU and repaints. Note the interaction with S4 --
an idle app must still paint **zero** frames, so a reduced-motion app must be at
least as still as an animated one, not more.

Nothing here is imported at package import time. `install_niki_glyphs()` in
`entry.py` binds it, and only when the process was launched as `niki` -- so
`dcode` keeps upstream's glyphs exactly.
"""

from __future__ import annotations

import os

NIKI_REDUCED_MOTION_ENV = "NIKI_REDUCED_MOTION"

#: Default frame cadence, in seconds.
CADENCE = 0.12

#: Faster cadence for a turn that is still producing output.
CADENCE_ACTIVE = 0.10

#: Calmer cadence once a turn has run long enough that urgency reads as noise.
CADENCE_CALM = 0.20

#: Seconds after which a turn is considered long-running.
CALM_AFTER_SECONDS = 20.0

#: The rotating-quadrant motif, Unicode and ASCII.
NIKI_SPINNER_FRAMES: tuple[str, ...] = ("◐", "◓", "◑", "◒")
NIKI_SPINNER_FRAMES_ASCII: tuple[str, ...] = ("-", "|", "/", "\\")

#: The single frame shown when motion is reduced.
STATIC_FRAME = "◐"
STATIC_FRAME_ASCII = "*"

_TRUTHY = frozenset({"1", "true", "yes", "on"})


def spinner_frames_for_this_terminal() -> tuple[str, ...]:
    """Resolve the frame sequence this terminal should get.

    Returns:
        A single static frame when motion is reduced, otherwise the Unicode or
        ASCII motif.
    """
    ascii_only = charset_is_ascii()
    if reduced_motion():
        return (STATIC_FRAME_ASCII if ascii_only else STATIC_FRAME,)
    return NIKI_SPINNER_FRAMES_ASCII if ascii_only else NIKI_SPINNER_FRAMES


def reduced_motion() -> bool:
    """Whether animation should be suppressed.

    Returns:
        `True` when `NIKI_REDUCED_MOTION` is truthy. `NO_MOTION` is honoured too,
        following the convention of the better-known `NO_COLOR`.
    """
    for name in (NIKI_REDUCED_MOTION_ENV, "NO_MOTION"):
        if os.environ.get(name, "").strip().lower() in _TRUTHY:
            return True
    return False


def charset_is_ascii() -> bool:
    """Whether the active terminal should get the ASCII glyph set.

    Returns:
        `True` when the encoded stdout cannot represent the Unicode motif, or
        when the charset is forced to ASCII. Mirrors aider's approach of probing
        the encoding rather than trusting a terminal-detection heuristic.
    """
    forced = os.environ.get("UI_CHARSET_MODE", "").strip().lower()
    if forced == "ascii":
        return True
    encoding = getattr(__import__("sys").stdout, "encoding", None) or ""
    if not encoding:
        return False
    try:
        NIKI_SPINNER_FRAMES[0].encode(encoding)
    except (UnicodeEncodeError, LookupError):
        return True
    return False


#: Why there is no `NikiGlyphs` class here. An earlier draft returned a narrow
#: stand-in carrying only `spinner_frames` and `reduced_motion`, which looked
#: like a clean delta and satisfied the type in places -- but the compositor
#: asks the glyph set for `.checkmark`, `.spinner_frames`, and more, so the TUI
#: died with `AttributeError` before it ever painted a composer. The fix is to
#: substitute upstream's FULL `Glyphs` with one field replaced, which
#: `install_niki_glyphs` does via `dataclasses.replace`. Narrow stand-ins for a
#: 34-field configuration object are a trap; record it so nobody repeats it.


def cadence_for(elapsed_seconds: float, *, active: bool = False) -> float:
    """Return the frame cadence for a turn at `elapsed_seconds`.

    Args:
        elapsed_seconds: How long the current turn has been running.
        active: Whether output is still arriving.

    Returns:
        Seconds between frames.
    """
    if active:
        return CADENCE_ACTIVE
    if elapsed_seconds >= CALM_AFTER_SECONDS:
        return CADENCE_CALM
    return CADENCE


def frame_at_tick(frames: tuple[str, ...], tick: int) -> tuple[int, str]:
    """Map a monotonically increasing tick to a ping-pong frame.

    Stateless on purpose: the caller owns the counter, which is what lets the
    animation timer be paused and resumed without the frame drifting out of sync
    with what is on screen. Pings and pongs rather than wrapping, so the sweep
    reverses at each end instead of jumping -- the reason the motion reads as a
    rotation rather than a loop.

    Args:
        frames: The frame sequence.
        tick: A counter that only ever increases while the animation runs.

    Returns:
        The frame index and the frame it selects.
    """
    if not frames:
        return 0, ""
    # Forward, then back through the interior frames only. Rebuilding the walk
    # each call would be O(n) per frame; the arithmetic below does the same job
    # by reflecting the index at the end of the run.
    count = len(frames)
    period = (count - 1) * 2 or 1
    index = tick % period
    if index >= count - 1:
        index = period - index
    index = min(max(index, 0), count - 1)
    return index, frames[index]


__all__ = [
    "CADENCE",
    "CADENCE_ACTIVE",
    "CADENCE_CALM",
    "CALM_AFTER_SECONDS",
    "NIKI_REDUCED_MOTION_ENV",
    "NIKI_SPINNER_FRAMES",
    "NIKI_SPINNER_FRAMES_ASCII",
    "STATIC_FRAME",
    "STATIC_FRAME_ASCII",
    "cadence_for",
    "charset_is_ascii",
    "frame_at_tick",
    "reduced_motion",
    "spinner_frames_for_this_terminal",
]
