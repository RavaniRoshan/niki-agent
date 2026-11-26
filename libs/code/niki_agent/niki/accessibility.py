"""Accessibility switches that change how the client renders.

The TUI assumes a screen that can be repainted in place. A screen reader reads
the stream linearly, so in-place rewrites are noise or silence: it hears the
same line repeatedly or misses the final one. `NIKI_SCREEN_READER=1` selects
the linear renderer instead -- one line per event, no cursor movement, no
spinner.
"""

from __future__ import annotations

import os

NIKI_SCREEN_READER_ENV = "NIKI_SCREEN_READER"

_TRUTHY = frozenset({"1", "true", "yes", "on"})


def screen_reader_enabled() -> bool:
    """Whether the user asked for linear, append-only output.

    Returns:
        `True` when `NIKI_SCREEN_READER` is truthy. The same spellings as
        `NIKI_REDUCED_MOTION` are accepted so one habit covers both switches.
    """
    return os.environ.get(NIKI_SCREEN_READER_ENV, "").strip().lower() in _TRUTHY


__all__ = ["NIKI_SCREEN_READER_ENV", "screen_reader_enabled"]
