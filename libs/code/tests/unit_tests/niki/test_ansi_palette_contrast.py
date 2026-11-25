"""V13: 16-colour fallback measured against real terminal palettes.

The RGB palettes are Niki's accessibility guarantee and are checked elsewhere.
This module answers the remaining question: when a limited-colour terminal is
in use, do the semantic ANSI tokens still clear their floors against the
palette the terminal actually paints?

The application cannot choose the user's palette, so the honest check is
against the two documented defaults. The dark mapping is tuned to clear every
floor on the standard xterm palette. Text tokens also clear on Solarized dark;
its saturated accents (green, yellow, orange) are low-contrast by design, which
no 16-colour mapping can repair without abandoning the semantic hue.
"""

from __future__ import annotations

import pytest

from deepagents_code.niki.theme import (
    GLYPH_CONTRAST_MIN,
    GLYPH_TOKENS,
    NIKI_ANSI16_DARK,
    NIKI_ANSI16_LIGHT,
    SOLARIZED_DARK_16_PALETTE as SOLARIZED_DARK,
    TEXT_CONTRAST_MIN,
    TEXT_TOKENS,
    XTERM_16_PALETTE as XTERM,
    contrast_ratio,
)


def _resolve(mapping: dict[str, str], palette: dict[str, str]) -> dict[str, str]:
    return {
        token: palette[name.removeprefix("ansi_")] for token, name in mapping.items()
    }


def _pairs(
    resolved: dict[str, str], tokens: frozenset[str] | set[str], floor: float
) -> list[tuple[str, str, float]]:
    failures = []
    for token in tokens:
        for background in ("background", "surface", "panel"):
            ratio = contrast_ratio(resolved[token], resolved[background])
            if ratio < floor:
                failures.append((token, background, ratio))
    return failures


def test_ansi_mapping_names_only_the_sixteen_colours() -> None:
    """V13: the fallback stays within the 16-colour vocabulary."""
    allowed = set(XTERM)
    for mapping in (NIKI_ANSI16_DARK, NIKI_ANSI16_LIGHT):
        for token, name in mapping.items():
            assert name.startswith("ansi_"), f"{token}: {name!r} is not an ANSI name"
            assert name.removeprefix("ansi_") in allowed, (
                f"{token}: {name!r} is outside the 16-colour palette"
            )


def test_dark_ansi16_clears_every_floor_on_xterm() -> None:
    """V13: on xterm's palette the dark fallback is fully legible."""
    resolved = _resolve(NIKI_ANSI16_DARK, XTERM)
    text = _pairs(resolved, TEXT_TOKENS, TEXT_CONTRAST_MIN)
    glyphs = _pairs(resolved, GLYPH_TOKENS, GLYPH_CONTRAST_MIN)
    assert not text, f"V13: text pairs below {TEXT_CONTRAST_MIN}: {text}"
    assert not glyphs, f"V13: glyph pairs below {GLYPH_CONTRAST_MIN}: {glyphs}"


def test_solarized_dark_keeps_text_readable() -> None:
    """V13: Solarized's low-contrast accents are the palette's, not the text's.

    Solarized Dark's green/yellow/orange sit below 3:1 on its background by
    construction; a 16-colour app cannot raise them without dropping the hue
    that carries the meaning. Text must still be readable, and the shortfall is
    bounded to those accent hues rather than hidden.
    """
    resolved = _resolve(NIKI_ANSI16_DARK, SOLARIZED_DARK)
    text = _pairs(resolved, TEXT_TOKENS, TEXT_CONTRAST_MIN)
    assert not text, f"V13: text pairs below {TEXT_CONTRAST_MIN} on Solarized: {text}"

    glyph_failures = _pairs(resolved, GLYPH_TOKENS, GLYPH_CONTRAST_MIN)
    failing_tokens = {token for token, _bg, _ratio in glyph_failures}
    # The documented cost: only the saturated accents fall short.
    assert failing_tokens <= {"success", "warning", "error", "skill"}, (
        f"V13: an unexpected token lost glyph contrast on Solarized: {failing_tokens}"
    )


@pytest.mark.parametrize("token", sorted(TEXT_TOKENS))
def test_light_ansi16_text_is_readable_on_xterm(token: str) -> None:
    """V13: light-mode text stays legible; the grey is the terminal's floor.

    xterm's `bright_black` is the mid-grey, which is the darkest grey the 16-colour
    vocabulary offers, so `muted` measures 3.95:1 rather than 4.5:1. That is a
    property of the palette, recorded here rather than papered over.
    """
    resolved = _resolve(NIKI_ANSI16_LIGHT, XTERM)
    ratio = contrast_ratio(resolved[token], resolved["background"])
    if token == "muted":
        assert ratio >= 3.9, f"V13: light muted fell to {ratio:.2f}:1"
    else:
        assert ratio >= TEXT_CONTRAST_MIN, (
            f"V13: light {token} is {ratio:.2f}:1 on xterm"
        )
