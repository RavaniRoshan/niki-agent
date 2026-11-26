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

#: High-contrast variant. For a bright room, a low-quality panel, or any
#: display where the default palette reads as muddy. Pushes foreground to pure
#: white and lifts every status hue, so the tightest pair is 6.22:1 rather than
#: the default's 4.99:1. Costs some of the "calm" the default is tuned for --
#: which is why this is a variant and not the default.
NIKI_CONTRAST: Final[dict[str, str]] = {
    "background": "#0B0C0E",
    "surface": "#131418",
    "panel": "#1B1D22",
    "foreground": "#FFFFFF",
    "muted": "#A8AEB8",
    "primary": "#8FB0FF",
    "secondary": "#9AA1AD",
    "accent": "#5FC7B8",
    "success": "#7FD69A",
    "warning": "#EFB457",
    "error": "#F07A75",
    "mode_bash": "#8FB0FF",
    "mode_command": "#9AA1AD",
    "mode_incognito": "#AFA0C2",
    "skill": "#C0A2E8",
    "skill_hover": "#D6BEF6",
    "tool": "#7FC4DE",
    "tool_hover": "#9BD8EC",
}

#: Low-luminance variant. For a dark room where the default's greys sit too
#: brightly against the terminal. Darker overall, with the accent held back so
#: it does not become the loudest thing on screen. Sits closest to its own
#: floors of the four, which is the honest cost of turning everything down.
NIKI_DIM: Final[dict[str, str]] = {
    "background": "#101216",
    "surface": "#171A1F",
    "panel": "#1F2229",
    "foreground": "#C9CDD4",
    "muted": "#8C929D",
    "primary": "#5C7FD6",
    "secondary": "#7B828D",
    "accent": "#3C8F84",
    "success": "#4E8F68",
    "warning": "#B07E33",
    "error": "#B85450",
    "mode_bash": "#5C7FD6",
    "mode_command": "#7B828D",
    "mode_incognito": "#8478A0",
    "skill": "#8F76B0",
    "skill_hover": "#A491C4",
    "tool": "#4E8AA2",
    "tool_hover": "#69A3B8",
}

#: Every Niki variant, by the name it registers under.
NIKI_PALETTES: Final[dict[str, dict[str, str]]] = {
    "niki": NIKI_DARK,
    "niki-light": NIKI_LIGHT,
    "niki-contrast": NIKI_CONTRAST,
    "niki-dim": NIKI_DIM,
}

# Textual ANSI color names use an `ansi_` prefix. Set `NIKI_COLOR_DEPTH=16` to
# register semantic 16-color variants while keeping the measured RGB palettes
# as the default.
NIKI_ANSI16_DARK: Final[dict[str, str]] = {
    "background": "ansi_black",
    "surface": "ansi_black",
    # `ansi_bright_black` is a mid-grey (#7F7F7F on xterm), so using it behind
    # status text drops several hues under their floors. A 16-colour terminal
    # has no subtle panel tint; the border and padding carry the separation.
    "panel": "ansi_black",
    "foreground": "ansi_bright_white",
    "muted": "ansi_white",
    "primary": "ansi_bright_blue",
    "secondary": "ansi_bright_white",
    "accent": "ansi_bright_cyan",
    "success": "ansi_bright_green",
    "warning": "ansi_bright_yellow",
    "error": "ansi_bright_red",
    "mode_bash": "ansi_bright_blue",
    "mode_command": "ansi_white",
    "mode_incognito": "ansi_bright_magenta",
    "skill": "ansi_bright_magenta",
    "skill_hover": "ansi_bright_white",
    "tool": "ansi_bright_cyan",
    "tool_hover": "ansi_bright_white",
}

NIKI_ANSI16_LIGHT: Final[dict[str, str]] = {
    "background": "ansi_bright_white",
    "surface": "ansi_white",
    "panel": "ansi_white",
    "foreground": "ansi_black",
    # On white, the saturated green (#00CD00) and yellow (#CDCD00) sit near
    # 1.7:1 -- unreadable at any size. This mapping keeps every token above its
    # floor by spending hue distinctness instead: the glyph and the word carry
    # the meaning. `ansi_red` is skipped for warning because `ansi_bright_red`
    # is the only red that clears the glyph floor while staying distinct from
    # plain text.
    "muted": "ansi_black",
    "primary": "ansi_blue",
    "secondary": "ansi_bright_black",
    "accent": "ansi_magenta",
    "success": "ansi_blue",
    "warning": "ansi_magenta",
    "error": "ansi_bright_red",
    "mode_bash": "ansi_blue",
    "mode_command": "ansi_bright_black",
    "mode_incognito": "ansi_magenta",
    "skill": "ansi_magenta",
    "skill_hover": "ansi_blue",
    "tool": "ansi_blue",
    "tool_hover": "ansi_black",
}

NIKI_ANSI16_PALETTES: Final[dict[str, dict[str, str]]] = {
    "niki": NIKI_ANSI16_DARK,
    "niki-light": NIKI_ANSI16_LIGHT,
    "niki-contrast": NIKI_ANSI16_DARK,
    "niki-dim": NIKI_ANSI16_DARK,
}

#: Xterm's documented 16-colour defaults. Reference data, not a Niki palette:
#: the tests use it to measure the ANSI fallback against the palette a real
#: terminal is most likely to paint.
XTERM_16_PALETTE: Final[dict[str, str]] = {
    "black": "#000000",
    "red": "#CD0000",
    "green": "#00CD00",
    "yellow": "#CDCD00",
    "blue": "#0000EE",
    "magenta": "#CD00CD",
    "cyan": "#00CDCD",
    "white": "#E5E5E5",
    "bright_black": "#7F7F7F",
    "bright_red": "#FF0000",
    "bright_green": "#00FF00",
    "bright_yellow": "#FFFF00",
    "bright_blue": "#5C5CFF",
    "bright_magenta": "#FF00FF",
    "bright_cyan": "#00FFFF",
    "bright_white": "#FFFFFF",
}

#: Solarized Dark's sixteen slots (Ethan Schoonover's published values). Like
#: `XTERM_16_PALETTE`, this is reference data used only to measure the fallback.
SOLARIZED_DARK_16_PALETTE: Final[dict[str, str]] = {
    "black": "#073642",
    "red": "#DC322F",
    "green": "#859900",
    "yellow": "#B58900",
    "blue": "#268BD2",
    "magenta": "#D33682",
    "cyan": "#2AA198",
    "white": "#EEE8D5",
    "bright_black": "#002B36",
    "bright_red": "#CB4B16",
    "bright_green": "#586E75",
    "bright_yellow": "#657B83",
    "bright_blue": "#839496",
    "bright_magenta": "#6C71C4",
    "bright_cyan": "#93A1A1",
    "bright_white": "#FDF6E3",
}


def _xterm_256_palette() -> dict[int, str]:
    """Build xterm's documented 256-colour palette, indexed by colour number.

    Slots 0-15 are the system colours above, 16-231 the 6x6x6 cube, and
    232-255 the greyscale ramp. `XTERM_16_PALETTE` is indexed by name, so the
    sixteen system slots are taken in the order the escape codes use.

    Returns:
        Colour number to hex value, for all 256 slots.
    """
    palette: dict[int, str] = {}
    system = tuple(XTERM_16_PALETTE.values())
    palette.update(enumerate(system))
    levels = (0, 95, 135, 175, 215, 255)
    index = 16
    for red in levels:
        for green in levels:
            for blue in levels:
                palette[index] = f"#{red:02X}{green:02X}{blue:02X}"
                index += 1
    for step in range(24):
        value = 8 + step * 10
        palette[232 + step] = f"#{value:02X}{value:02X}{value:02X}"
    return palette


#: Reference data, not a Niki palette: the tests quantize truecolor down to
#: this table to prove a 256-colour terminal still clears every contrast floor.
XTERM_256_PALETTE: Final[dict[int, str]] = _xterm_256_palette()

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
    "NIKI_ANSI16_DARK",
    "NIKI_ANSI16_LIGHT",
    "NIKI_ANSI16_PALETTES",
    "NIKI_CONTRAST",
    "NIKI_DARK",
    "NIKI_DIM",
    "NIKI_LIGHT",
    "NIKI_PALETTES",
    "NIKI_THEME_NAME",
    "SOLARIZED_DARK_16_PALETTE",
    "TEXT_CONTRAST_MIN",
    "TEXT_TOKENS",
    "XTERM_16_PALETTE",
    "XTERM_256_PALETTE",
    "contrast_ratio",
    "contrast_violations",
    "relative_luminance",
]
