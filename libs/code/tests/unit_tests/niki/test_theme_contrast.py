"""V13/V14: every bundled theme clears its contrast floors at every depth.

The RGB palettes are the truecolor path. A 256-colour terminal quantizes those
same values, so the test quantizes them here and re-measures rather than
trusting the truecolor result to survive a downstep. The 16-colour fallbacks
are measured against xterm's documented defaults, the palette a limited
terminal is most likely to paint.

A light 16-colour terminal has no readable saturated green or yellow on white
(xterm's defaults for both fall near 1.7:1). The light ANSI mapping therefore
spends hue distinctness to keep every token above its floor: glyphs and words
carry the meaning, which is the accessibility rule this table exists to
protect.
"""

from __future__ import annotations

import pytest

from niki_agent.niki.theme import (
    NIKI_ANSI16_PALETTES,
    NIKI_PALETTES,
    XTERM_16_PALETTE,
    XTERM_256_PALETTE,
    contrast_violations,
)


def _rgb(color: str) -> tuple[int, int, int]:
    return (int(color[1:3], 16), int(color[3:5], 16), int(color[5:7], 16))


def _nearest_256(color: str) -> str:
    target = _rgb(color)
    return min(
        XTERM_256_PALETTE.values(),
        key=lambda candidate: sum(
            (a - b) ** 2 for a, b in zip(target, _rgb(candidate), strict=True)
        ),
    )


def _resolve_ansi16(mapping: dict[str, str]) -> dict[str, str]:
    return {
        token: XTERM_16_PALETTE[name.removeprefix("ansi_")]
        for token, name in mapping.items()
    }


def test_rgb_palettes_clear_every_floor() -> None:
    """The truecolor path meets its floors in every bundled theme."""
    failures = {
        name: contrast_violations(palette) for name, palette in NIKI_PALETTES.items()
    }
    assert {name: found for name, found in failures.items() if found} == {}


def test_256_quantized_palettes_clear_every_floor() -> None:
    """Downstepping truecolor to xterm-256 does not drop a pair below its floor."""
    failures = {}
    for name, truecolor in NIKI_PALETTES.items():
        quantized = {token: _nearest_256(color) for token, color in truecolor.items()}
        found = contrast_violations(quantized)
        if found:
            failures[name] = found
    assert failures == {}


def test_ansi16_fallbacks_clear_every_floor_on_xterm() -> None:
    """The 16-colour path is legible on the documented xterm defaults."""
    failures = {
        name: contrast_violations(_resolve_ansi16(mapping))
        for name, mapping in NIKI_ANSI16_PALETTES.items()
    }
    assert {name: found for name, found in failures.items() if found} == {}


@pytest.mark.parametrize("name", sorted(NIKI_PALETTES))
def test_palettes_define_the_same_tokens(name: str) -> None:
    """A theme missing a token would silently fall back to a terminal default."""
    assert set(NIKI_PALETTES[name]) == set(NIKI_PALETTES["niki"])
    assert set(NIKI_ANSI16_PALETTES[name]) == set(NIKI_PALETTES["niki"])
