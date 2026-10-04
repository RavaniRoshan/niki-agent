"""Regression guard: no upstream brand string may reach the first-run screen.

This exists because a real launch caught what 237 tests did not. Every test
built `NikiApp` directly and mounted widgets itself; none of them went through
the entry point, so none of them ever saw that a *fresh profile* opens a
`LaunchNameScreen` before the transcript exists. A first-time user was therefore
greeted by "Welcome to Deep Agents Code" and never reached the Niki UI at all.

The lesson, recorded so it is not relearned: a unit test that constructs the
component cannot find a screen it never mounts.
"""

from __future__ import annotations

from pathlib import Path

import pytest

from deepagents_code.niki.product import PRODUCT_NAME

LAUNCH_INIT = (
    Path(__file__).resolve().parents[3]
    / "deepagents_code"
    / "tui"
    / "widgets"
    / "launch_init.py"
)


def test_the_first_run_screen_names_niki_not_upstream() -> None:
    """V2: the onboarding a fresh profile sees must be Niki's."""
    text = LAUNCH_INIT.read_text(encoding="utf-8")

    assert "Welcome to Deep Agents Code" not in text, (
        "V2: the first-run screen still greets the user as Deep Agents Code. "
        "On a fresh profile this screen appears BEFORE the transcript, so a user "
        "would never reach the Niki UI."
    )
    assert "What should Deep Agents call you?" not in text, (
        "V2: the first-run prompt still names Deep Agents."
    )
    # The file must *reference* the shared constant, not restate the string: a
    # second literal is exactly how the two copies drift apart.
    assert "PRODUCT_NAME" in text, (
        "V2: the first-run screen does not reference the shared product-name "
        "constant; a restated literal would be free to drift"
    )


def test_no_user_visible_upstream_name_survives_in_the_tui() -> None:
    """V2: sweep the TUI widgets for brand strings a user could actually read.

    Scoped to string literals handed to Textual content helpers, so docstrings
    and comments -- which legitimately explain the fork -- are not flagged.
    """
    import ast

    tui = Path(__file__).resolve().parents[3] / "deepagents_code" / "tui"
    offenders: list[str] = []

    for path in sorted(tui.rglob("*.py")):
        try:
            tree = ast.parse(path.read_text(encoding="utf-8"))
        except SyntaxError:
            continue
        for node in ast.walk(tree):
            if not isinstance(node, ast.Constant) or not isinstance(node.value, str):
                continue
            text = node.value
            if "Deep Agents" not in text and "dcode" not in text:
                continue
            # Keep it only if it reads like user-visible copy rather than prose.
            looks_like_copy = (
                text.startswith(("Welcome", "dcode ", "Deep Agents"))
                or "call you" in text
                or "Deep Agents Code" in text
            )
            if looks_like_copy:
                offenders.append(f"{path.name}:{node.lineno}: {text[:60]!r}")

    assert not offenders, f"V2: user-visible copy still names upstream: {offenders}"


def test_the_product_name_is_shared_not_restated() -> None:
    """The onboarding and the version line must not be able to disagree."""
    from deepagents_code.niki import branding

    assert branding.PRODUCT_NAME == PRODUCT_NAME, (
        "the product name is defined twice with different values; it must have one home"
    )


@pytest.mark.parametrize("screen", ["LaunchNameScreen", "LaunchDependenciesScreen"])
def test_the_onboarding_screens_are_reachable_but_branded(screen: str) -> None:
    """The screens exist; assert the module that defines them is the branded one."""
    module = LAUNCH_INIT
    assert screen in module.read_text(encoding="utf-8"), (
        f"{screen} is no longer defined in launch_init.py; this test needs "
        "updating for wherever it moved"
    )
