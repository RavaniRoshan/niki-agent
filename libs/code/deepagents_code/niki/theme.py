"""Niki Agent visual identity: the one place a Niki color may be written.

Every color Niki uses lives in this module. Nothing else in the fork may spell a
hex literal, a named color, or an `rgb()` -- `tests/unit_tests/niki/test_lint_rules.py`
enforces that. Keeping the tokens here is what lets V1 be a lint rule instead of
a review habit, and it means a palette change is one edit in one file.

**Direction, not values.** The owner asked for a palette inspired by two tools
whose colour systems were read from source: Codex CLI (`codex-rs/tui/src/style.rs`)
and Kimi Code (`apps/kimi-code/src/tui/theme/colors.ts`). Three ideas were taken,
and **no hex value was** -- the brief forbids reproducing another tool's palette,
and `test_the_palette_is_not_a_copy` enforces that against every value either
tool uses:

1. **A flat, untinted grey scale** (Kimi). `foreground`, `muted` and `secondary`
   carry no hue of their own, so colour is spent only on meaning. An earlier
   Niki palette used a warm-tinted grey, which made "secondary" look like a
   fourth accent.
2. **One accent, four status hues, each with one job** (both). `error` is the
   only hue allowed to mean removal or failure, which is what lets a diff read
   without relying on position.
3. **Status colours desaturated relative to the accent** (Codex's diff
   backgrounds are deliberately subtle so they never fight syntax colours). A
   saturated status colour competes with the accent and the accent stops
   working.

The base is cool rather than warm, and every pair is measured before it ships.

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
    "background": "#14161A",
    "surface": "#1B1E25",
    "panel": "#232831",
    "foreground": "#E4E7EC",
    "muted": "#9AA1AD",
    "primary": "#6B8EF2",
    "secondary": "#8B93A3",
    "accent": "#43AFA0",
    "success": "#5FAE7A",
    "warning": "#D99A3E",
    "error": "#D2605C",
    "mode_bash": "#6B8EF2",
    "mode_command": "#8B93A3",
    "mode_incognito": "#9B8AAE",
    "skill": "#A98BD0",
    "skill_hover": "#C0A6DE",
    "tool": "#5FA8C4",
    "tool_hover": "#7FC2D8",
}

#: Light palette. Same hue relationships, re-weighted for a light background.
NIKI_LIGHT: Final[dict[str, str]] = {
    "background": "#FAFAFB",
    "surface": "#F1F2F4",
    "panel": "#E7E9ED",
    "foreground": "#1F2328",
    "muted": "#5C636E",
    "primary": "#3355CC",
    "secondary": "#4B535F",
    "accent": "#1F7A70",
    "success": "#2E7D4F",
    "warning": "#8A5E12",
    "error": "#A83232",
    "mode_bash": "#3355CC",
    "mode_command": "#4B535F",
    "mode_incognito": "#6B4E86",
    "skill": "#6B3FA0",
    "skill_hover": "#573089",
    "tool": "#1F6C86",
    "tool_hover": "#17566B",
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
