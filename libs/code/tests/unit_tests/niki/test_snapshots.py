"""Visual snapshots at the four layout tiers the checklist names.

Widths are the ones V3 requires: 50 (compact), 80 (standard), 120 (roomy), and
160 (the upper tier that must not look stretched). Each tier gets the same app
state, so a difference between two snapshots is a layout difference and nothing
else.

Snapshots are generated with `--snapshot-update`. They are committed, so a
regression shows up as a diff in review rather than as a passing test.
"""

from __future__ import annotations

from pathlib import Path

import pytest

APP_PATH = str(Path(__file__).parent / "snapshot_app.py")

#: (width, height) for each tier, plus whether the app must show its resize hint.
TIERS = [(50, 16), (80, 24), (120, 38), (160, 45)]


@pytest.mark.parametrize(("width", "height"), TIERS, ids=[f"{w}x{h}" for w, h in TIERS])
def test_first_run_snapshot(snap_compare, width: int, height: int) -> None:
    """V3/V12: the first-run state renders identically at every tier."""
    assert snap_compare(APP_PATH, terminal_size=(width, height))


@pytest.mark.parametrize(("width", "height"), TIERS, ids=[f"{w}x{h}" for w, h in TIERS])
def test_snapshot_with_text_typed(snap_compare, width: int, height: int) -> None:
    """V5: the composer takes focus and shows typed text at every tier."""
    assert snap_compare(
        APP_PATH,
        terminal_size=(width, height),
        press=["h", "e", "l", "l", "o"],
    )


@pytest.mark.parametrize("width", [50, 120], ids=["50", "120"])
def test_snapshot_with_slash_menu_open(snap_compare, width: int) -> None:
    """K6: the slash popup opens over the transcript without resizing it."""
    assert snap_compare(APP_PATH, terminal_size=(width, 24), press=["/"])


@pytest.mark.parametrize("width", [50, 120], ids=["50", "120"])
def test_snapshot_with_help_open(snap_compare, width: int) -> None:
    """K8: contextual help renders and stays on screen at narrow and roomy widths."""
    assert snap_compare(APP_PATH, terminal_size=(width, 24), press=["question_mark"])
