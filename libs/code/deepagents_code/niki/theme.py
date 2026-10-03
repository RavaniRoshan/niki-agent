"""Niki Agent visual identity: the one place a Niki color may be written.

Every color Niki uses lives in this module. Nothing else in the fork may spell a
hex literal, a named color, or an `rgb()` -- `tests/unit_tests/niki/test_lint_rules.py`
enforces that. Keeping the tokens here is what lets V1 be a lint rule instead of
a review habit, and it means a palette change is one edit in one file.

The accent is a periwinkle (`#8FA3F5`). It is deliberately not orange and not
terminal green, so it cannot be mistaken for another tool's chrome, and it stays
calm against a warm-neutral base rather than glowing the way a saturated cyan
would. Success, warning, and error are desaturated on purpose: they carry status
and nothing else, and a status color that competes with the accent for attention
makes the accent useless.

Contrast is not asserted by hand here. `contrast_ratio` is the same function the
test uses, so `test_theme_contrast.py` and this docstring cannot drift.
"""

from __future__ import annotations

from typing import Final

#: WCAG's sRGB linearization breakpoint.
_SRGB_BREAKPOINT: Final = 0.04045

#: Distinguishes a text floor from a glyph floor when reporting a violation.
_TEXT_FLOOR: Final = 4.5

NIKI_THEME_NAME: Final = "niki"
"""Registry key for the Niki theme, matching `theme.DEFAULT_THEME` conventions."""


def _rgb(color: str) -> tuple[float, float, float]:
    """Parse a `#RRGGBB` string into 0-255 float channels.

    Returns:
        The red, green, and blue channels on a 0-255 scale.
    """
    return (
        float(int(color[1:3], 16)),
        float(int(color[3:5], 16)),
        float(int(color[5:7], 16)),
    )


def relative_luminance(color: str) -> float:
    """WCAG relative luminance of a `#RRGGBB` color.

    Returns:
        Relative luminance in 0.0-1.0, where 0.0 is black and 1.0 is white.
    """
    channels = []
    for raw in _rgb(color):
        srgb = raw / 255
        channels.append(
            srgb / 12.92
            if srgb <= _SRGB_BREAKPOINT
            else ((srgb + 0.055) / 1.055) ** 2.4
        )
    red, green, blue = channels
    return 0.2126 * red + 0.7152 * green + 0.0722 * blue


def contrast_ratio(foreground: str, background: str) -> float:
    """WCAG contrast ratio between two `#RRGGBB` colors.

    Returns:
        The contrast ratio, from 1.0 (identical) to 21.0 (black on white).
    """
    first = relative_luminance(foreground)
    second = relative_luminance(background)
    lighter, darker = max(first, second), min(first, second)
    return (lighter + 0.05) / (darker + 0.05)


#: Dark-first palette. Warm-neutral base, one periwinkle accent, desaturated status.
NIKI_DARK: Final[dict[str, str]] = {
    "background": "#17161A",
    "surface": "#201F25",
    "panel": "#26252C",
    "foreground": "#E9E7E4",
    "muted": "#A19C94",
    "primary": "#8FA3F5",
    "secondary": "#9AA5B8",
    "accent": "#6FC5BE",
    "success": "#7FB685",
    "warning": "#D7B168",
    "error": "#E08A8A",
    "mode_bash": "#8FA3F5",
    "mode_command": "#9AA5B8",
    "mode_incognito": "#B9A0C9",
    "skill": "#C0A3E8",
    "skill_hover": "#D2BCF0",
    "tool": "#7FBFD4",
    "tool_hover": "#A6D8E6",
}

#: Light palette. Same hue relationships, re-weighted for a light background.
NIKI_LIGHT: Final[dict[str, str]] = {
    "background": "#FAF9F7",
    "surface": "#F2F0EC",
    "panel": "#E9E6E0",
    "foreground": "#26242A",
    "muted": "#625E57",
    "primary": "#3F51B5",
    "secondary": "#4C5568",
    "accent": "#1F7A73",
    "success": "#2F7A46",
    "warning": "#8A6410",
    "error": "#B03A3A",
    "mode_bash": "#3F51B5",
    "mode_command": "#4C5568",
    "mode_incognito": "#6B4E86",
    "skill": "#6B3FA0",
    "skill_hover": "#573089",
    "tool": "#1F6C80",
    "tool_hover": "#175A6B",
}

#: Which pairs must clear which WCAG level. Text carries meaning and needs 4.5:1;
#: glyphs, borders, and rules are UI and need 3:1.
TEXT_TOKENS: Final[frozenset[str]] = frozenset({"foreground", "muted"})
GLYPH_TOKENS: Final[frozenset[str]] = frozenset(
    {"primary", "secondary", "accent", "success", "warning", "error", "skill", "tool"}
)

#: Every token is checked against the base background it renders on.
_BACKGROUNDS: Final[tuple[str, ...]] = ("background", "surface", "panel")

TEXT_CONTRAST_MIN: Final = 4.5
GLYPH_CONTRAST_MIN: Final = 3.0


def contrast_violations(
    palette: dict[str, str],
) -> list[tuple[str, str, str, float, float]]:
    """Return every (token, background, requirement, actual, required) that fails.

    `muted` is held to 4.5:1 because Niki uses it for timestamps and secondary
    labels -- text a user reads, not a decorative glyph.
    """
    minimum_for = {
        **dict.fromkeys(TEXT_TOKENS, TEXT_CONTRAST_MIN),
        **dict.fromkeys(GLYPH_TOKENS, GLYPH_CONTRAST_MIN),
    }
    violations: list[tuple[str, str, str, float, float]] = []
    for token, required in minimum_for.items():
        for background_token in _BACKGROUNDS:
            actual = contrast_ratio(palette[token], palette[background_token])
            if actual < required:
                kind = "text" if required == TEXT_CONTRAST_MIN else "glyph"
                violations.append((token, background_token, kind, actual, required))
    return violations


__all__ = [
    "GLYPH_CONTRAST_MIN",
    "GLYPH_TOKENS",
    "NIKI_DARK",
    "NIKI_LIGHT",
    "NIKI_THEME_NAME",
    "TEXT_CONTRAST_MIN",
    "TEXT_TOKENS",
    "contrast_ratio",
    "contrast_violations",
    "relative_luminance",
]
