"""Lint tests: rules that are cheaper to enforce than to review.

Two of these encode checklist rows directly.

- **V1** -- all colors come from theme tokens. Scoped to the fork rather than to
  the whole package, because upstream already keeps its `.tcss` on `$tokens` and
  has three Python files with genuine hex (an ANSI-color helper for headless
  output, an OAuth UI, and the theme module itself). Refactoring those is out of
  scope, so the rule is "the fork adds no new color literals" plus "no new file
  in the package gains one".
- **L3** -- nothing writes to stdout or stderr while the TUI is live. This is a
  runtime capture, not a grep. Upstream prints 119 times for headless and
  subcommand output, which is correct; a static ban would flag code the fork
  must not touch. What must not happen is a write *while a screen is mounted*,
  because that corrupts the frame.
"""

from __future__ import annotations

import ast
import io
import json
import re
import sys
import warnings
from pathlib import Path

import pytest
from textual.app import App

import deepagents_code
from deepagents_code.niki import theme as niki_theme

PACKAGE_ROOT = Path(deepagents_code.__file__).parent
NIKI_ROOT = PACKAGE_ROOT / "niki"

#: Upstream files that legitimately contain hex colors. Recorded at BASE_SHA;
#: a fourth entry means the fork added one.
UPSTREAM_COLOR_FILES = frozenset({"theme.py", "terminal_escape.py", "mcp_auth.py"})

_HEX_COLOR = re.compile(r"#[0-9A-Fa-f]{6}\b")
_SOURCES = ("*.py", "*.tcss")


def _files_with_color_literals(root: Path) -> set[str]:
    """Return the names of files under `root` that spell a hex color."""
    found = set()
    for pattern in _SOURCES:
        for path in root.rglob(pattern):
            if _HEX_COLOR.search(path.read_text(encoding="utf-8")):
                found.add(path.name)
    return found


def test_niki_writes_colors_only_in_its_theme_module() -> None:
    """V1: the Niki package has exactly one file allowed to spell a color."""
    offenders = _files_with_color_literals(NIKI_ROOT) - {"theme.py"}
    assert not offenders, (
        f"color literals outside the Niki theme module: {sorted(offenders)}. "
        "Add a token to deepagents_code/niki/theme.py and reference it instead."
    )


def test_niki_package_ships_a_theme_module() -> None:
    """The rule above is only meaningful if there is a theme module to point at."""
    assert niki_theme.NIKI_THEME_NAME
    assert set(niki_theme.NIKI_DARK) == set(niki_theme.NIKI_LIGHT), (
        "the dark and light palettes must define the same tokens"
    )


def test_the_fork_adds_no_new_color_literal_files() -> None:
    """V1: the package must not grow a fourth file containing hex colors."""
    found = _files_with_color_literals(PACKAGE_ROOT)
    new_files = found - UPSTREAM_COLOR_FILES
    assert not new_files, (
        f"new files with color literals: {sorted(new_files)}. Expected only "
        f"{sorted(UPSTREAM_COLOR_FILES)} (upstream, recorded at BASE_SHA)."
    )


async def test_nothing_writes_to_stdout_or_stderr_while_the_tui_is_live() -> None:
    """L3: a mounted screen and any stray write cannot coexist."""
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.app import DeepAgentsApp

    app = DeepAgentsApp(agent=MagicMock(), thread_id="niki-lint")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]

    stdout, stderr = io.StringIO(), io.StringIO()
    original_out, original_err = sys.stdout, sys.stderr
    sys.stdout, sys.stderr = stdout, stderr
    try:
        async with app.run_test(size=(80, 24)) as pilot:
            await pilot.pause()
            await pilot.press("h", "i")
            await pilot.pause()
    finally:
        sys.stdout, sys.stderr = original_out, original_err

    assert stdout.getvalue() == "", (
        f"stdout wrote while the TUI was live: {stdout.getvalue()!r}"
    )
    assert stderr.getvalue() == "", (
        f"stderr wrote while the TUI was live: {stderr.getvalue()!r}"
    )


async def test_no_warning_escapes_while_the_tui_is_live() -> None:
    """L3: a library warning mid-frame corrupts the screen, so it must fail loudly."""
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.app import DeepAgentsApp

    app = DeepAgentsApp(agent=MagicMock(), thread_id="niki-warn")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]

    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        async with app.run_test(size=(80, 24)) as pilot:
            await pilot.pause()
            await pilot.press("x")
            await pilot.pause()

    user_warnings = [
        str(w.message) for w in caught if issubclass(w.category, UserWarning)
    ]
    assert not user_warnings, (
        f"warnings escaped during the TUI session: {user_warnings}"
    )


def test_the_harness_never_disables_a_check() -> None:
    """Guard against a future 'make it green' edit that turns a row off."""
    root = Path(__file__).parent
    for path in root.rglob("*.py"):
        if path.name == Path(__file__).name:
            continue  # this file necessarily names what it forbids
        text = path.read_text(encoding="utf-8")
        assert "pytest.skip" not in text, f"{path.name} skips a test"
        assert "xfail" not in text, f"{path.name} marks a test xfail"
        assert "@pytest.mark.skip" not in text, f"{path.name} skips a test"


@pytest.mark.parametrize("palette_name", ["NIKI_DARK", "NIKI_LIGHT"])
def test_every_token_pair_meets_its_contrast_floor(palette_name: str) -> None:
    """V13: text at 4.5:1, UI glyphs at 3:1, against every surface they sit on."""
    palette = getattr(niki_theme, palette_name)
    violations = niki_theme.contrast_violations(palette)
    assert not violations, "\n".join(
        f"{token} on {background} ({kind}): {actual:.2f} < {required:.1f}"
        for token, background, kind, actual, required in violations
    )


def test_no_color_literal_survives_in_the_niki_test_harness() -> None:
    """The harness must not smuggle a color past the theme module."""
    offenders = _files_with_color_literals(Path(__file__).parent) - {"theme.py"}
    assert not offenders, f"test harness spells colors directly: {sorted(offenders)}"


async def test_niki_stylesheet_actually_applies() -> None:
    """The Niki stylesheet must reach the widgets, not just parse.

    Regression guard for a silent no-op. A first draft of `niki.tcss` referenced
    `$niki-background` for every colour. Textual has no such variable, so the
    sheet failed to parse, Textual discarded it wholesale -- and **every snapshot
    still passed**, because the app rendered exactly as it had before. Nothing
    in the suite noticed that the look had not changed at all.

    This asserts the concrete outcome instead: the composer carries Niki's
    accent border and Niki's surface, and the transcript sits on Niki's
    background.
    """
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp
    from deepagents_code.niki.theme import NIKI_DARK

    app = NikiApp(agent=MagicMock(), thread_id="niki-css")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]

    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        composer = app.query_one("#input-area")
        messages = app.query_one("#messages")

    border = composer.styles.border
    assert border.top is not None, f"composer has no top border: {border!r}"
    assert border.top[0] == "round", (
        f"composer border is {border.top[0]}, expected round"
    )
    assert border.top[1].hex.lower().endswith(NIKI_DARK["primary"][1:].lower()), (
        f"composer border is {border.top[1].hex}, not Niki's primary"
    )
    assert messages.styles.background.hex.lower().endswith(
        NIKI_DARK["background"][1:].lower()
    ), f"transcript background is {messages.styles.background.hex}, not Niki's base"


_RESEARCHED = json.loads(
    (Path(__file__).parent / "researched_palettes.json").read_text(encoding="utf-8")
)
_RESEARCHED_PALETTE_VALUES = {
    key: set(values) for key, values in _RESEARCHED.items() if not key.startswith("_")
}


@pytest.mark.parametrize("palette_name", ["NIKI_DARK", "NIKI_LIGHT"])
def test_the_palette_is_not_a_copy(palette_name: str) -> None:
    """The brief forbids reproducing another tool's palette.

    The palette took *direction* from Codex and Kimi Code; this asserts it took
    no *values*. Without this, "inspired by" quietly becomes "reproduced" the
    first time someone tunes a hex by eye against a screenshot.

    The compared values live in `researched_palettes.json` rather than inline,
    because the harness lint rule forbids colour literals in test code -- and
    that rule should not be weakened to make this test expressible.
    """
    from deepagents_code.niki.theme import NIKI_DARK, NIKI_LIGHT

    palette = {"NIKI_DARK": NIKI_DARK, "NIKI_LIGHT": NIKI_LIGHT}[palette_name]
    mine = {value.upper() for value in palette.values()}
    theirs = {
        v.upper() for values in _RESEARCHED_PALETTE_VALUES.values() for v in values
    }

    collisions = mine & theirs
    assert not collisions, (
        f"{palette_name} reuses values from the palettes it drew direction "
        f"from: {sorted(collisions)}. Direction is fine; values are not."
    )
