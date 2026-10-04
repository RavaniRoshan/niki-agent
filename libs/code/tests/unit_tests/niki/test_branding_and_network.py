"""V2 (branding) and L6 (outbound network) as executable checks.

V2 is a naming discipline, so it is enforced two ways: the Niki package's own
source must not spell an upstream name, and the committed first-run frame must
not show one either.

L6 is a policy, so it is enforced by asserting the *defaults*: Niki's three
network behaviors are off unless an owner opts in by name.
"""

from __future__ import annotations

import ast
import re
from pathlib import Path
from typing import TYPE_CHECKING

import pytest

from deepagents_code.niki import branding, network

if TYPE_CHECKING:
    from collections.abc import Callable

NIKI_ROOT = Path(branding.__file__).parent

#: Strings that must not appear in Niki's own user-visible source.
UPSTREAM_NAMES = ("Deep Agents", "dcode")

#: The one place attribution is required rather than forbidden: `--version`.
ALLOWED_SOURCE_EXCEPTIONS = ("branding.py",)

#: Frames committed by `test_snapshots.py`. Checked for user-visible leaks.
PACKAGE_ROOT = NIKI_ROOT.parents[1]
SNAPSHOT_DIR = PACKAGE_ROOT / "tests" / "unit_tests" / "niki" / "__snapshots__"


def test_version_line_names_niki_and_credits_deep_agents() -> None:
    """`--version` must say Niki Agent and still credit what it is built on."""
    line = branding.version_line()
    assert line.startswith("Niki Agent "), line
    assert line.endswith(", built on Deep Agents"), line
    assert re.match(r"^Niki Agent \d", line), f"no version number in {line!r}"


def test_product_version_never_imports_the_package() -> None:
    """`-v` must stay cheap; reading metadata beats importing the app."""
    assert branding.product_version() != "unknown"
    assert branding.product_version() == branding.product_version()


@pytest.mark.parametrize("name", UPSTREAM_NAMES)
def test_niki_source_does_not_spell_upstream_names(name: str) -> None:
    """V2: Niki's own code must not show an upstream name to a user.

    Checked against parsed string *literals*, not raw text. Prose about the fork
    -- docstrings and comments explaining that `dcode` still works -- is exactly
    what a future reader needs and must not be flagged. A literal is what can
    reach the screen.
    """
    offenders: list[str] = []
    for path in sorted(NIKI_ROOT.glob("*.py")):
        if path.name in ALLOWED_SOURCE_EXCEPTIONS:
            continue
        tree = ast.parse(path.read_text(encoding="utf-8"))
        # Any bare string expression is prose, not user-visible text. This
        # covers attribute docstrings that follow an assignment, where
        # `body[0]` is the assignment rather than the string.
        docstrings = {
            id(node.value)
            for node in ast.walk(tree)
            if isinstance(node, ast.Expr)
            and isinstance(node.value, ast.Constant)
            and isinstance(node.value.value, str)
        }

        offenders.extend(
            f"{path.name}:{node.lineno}"
            for node in ast.walk(tree)
            if isinstance(node, ast.Constant)
            and isinstance(node.value, str)
            and id(node) not in docstrings
            and name in node.value
        )
    assert not offenders, (
        f"upstream name {name!r} in Niki source at {offenders}; "
        "use branding.PRODUCT_NAME instead"
    )


def test_dcode_command_still_exists_and_is_unchanged() -> None:
    """The fork adds `niki`; it must not have disturbed `dcode`."""
    import tomllib

    pyproject = PACKAGE_ROOT / "pyproject.toml"
    scripts = tomllib.loads(pyproject.read_text(encoding="utf-8"))["project"]["scripts"]

    assert scripts["dcode"] == "deepagents_code:cli_main"
    assert scripts["deepagents-code"] == "deepagents_code:cli_main"
    assert scripts["niki"] == "deepagents_code.niki.entry:niki_main"


@pytest.mark.parametrize(
    ("policy", "owner_env"),
    [
        (network.update_check_allowed, network.NIKI_DISABLE_UPDATE_CHECK_ENV),
        (network.auto_update_allowed, network.NIKI_DISABLE_AUTO_UPDATE_ENV),
        (network.remote_config_allowed, network.NIKI_DISABLE_REMOTE_CONFIG_ENV),
    ],
)
def test_network_matches_upstream_by_default_and_can_be_disabled(
    policy: Callable[[], bool], owner_env: str, monkeypatch: pytest.MonkeyPatch
) -> None:
    """L6: Niki matches upstream -- on by default -- with an owner opt-out.

    The owner chose parity over the opt-in posture this originally shipped. See
    the module docstring in `network.py` for why the original reasoning was
    wrong: the distribution was never renamed, so upstream's upgrade command
    upgrades the package that provides `niki`.
    """
    monkeypatch.delenv(owner_env, raising=False)
    assert policy() is True, "Niki must match upstream's on-by-default posture"

    monkeypatch.setenv(owner_env, "1")
    assert policy() is False, "the owner opt-out must work"


def test_network_policy_never_overrides_an_explicit_owner_choice(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A value already in the environment is a decision, and must survive."""
    monkeypatch.delenv(network.NIKI_DISABLE_UPDATE_CHECK_ENV, raising=False)
    monkeypatch.setenv("DEEPAGENTS_CODE_NO_UPDATE_CHECK", "0")

    network.apply_niki_network_policy()

    import os

    assert os.environ["DEEPAGENTS_CODE_NO_UPDATE_CHECK"] == "0", (
        "the policy overwrote an explicit owner setting; it must only supply defaults"
    )


def test_the_upstream_install_script_is_identified_as_a_live_concern() -> None:
    """The one network path that is still wrong for a fork.

    `INSTALL_SCRIPT_COMMAND` fetches *upstream's* installer over curl. With
    auto-update now on by default, a Niki user can be offered a script that
    installs upstream's tooling. Asserted so the string cannot change without
    this test noticing.
    """
    command = network.upstream_install_script_command()

    assert "langch.in/dcode" in command, (
        f"the upstream install script changed to {command!r}; re-check whether it "
        "still installs upstream tooling rather than Niki"
    )


def test_snapshot_frames_do_not_show_the_upstream_product_name() -> None:
    """V2: the rendered first-run frame must not show 'Deep Agents'."""
    frames = sorted(SNAPSHOT_DIR.rglob("*.raw"))
    assert frames, (
        "no committed snapshots; run the snapshot suite with --snapshot-update"
    )

    offenders = []
    for frame in frames:
        text = frame.read_text(encoding="utf-8", errors="replace")
        for node in re.findall(r">([^<>]{2,})</text>", text):
            # SVG stores spaces as non-breaking entities.
            plain = (
                node.replace("&#160;", " ").replace("&gt;", ">").replace("&amp;", "&")
            )
            if "Deep Agents" in plain:
                offenders.append(f"{frame.name}: {plain!r}")
    assert not offenders, f"upstream product name visible in {offenders}"


def test_snapshot_frames_show_niki_agent() -> None:
    """V2: the rebrand must actually be on screen, not merely absent upstream."""
    frame = SNAPSHOT_DIR / "test_snapshots" / "test_first_run_snapshot[80x24].raw"
    assert frame.exists(), (
        f"missing {frame}; regenerate with --snapshot-update. A skipped check is "
        "worse than a failing one here."
    )
    text = frame.read_text(encoding="utf-8", errors="replace")
    assert "Niki&#160;Agent" in text or "Niki Agent" in text, (
        "the first-run frame does not show the Niki name"
    )


async def test_niki_theme_is_registered_and_active() -> None:
    """V1/V13: Niki's palette must reach the running app, not just the module."""
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp
    from deepagents_code.niki.theme import NIKI_DARK
    from deepagents_code.theme import get_theme_colors

    app = NikiApp(agent=MagicMock(), thread_id="niki-theme")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        available = set(app.available_themes)
        active = app.theme
        colors = get_theme_colors(app)

    assert "niki" in available, f"Niki theme not registered; got {sorted(available)}"
    assert "niki-light" in available, "no light palette registered for light terminals"
    assert active == "niki", f"active theme is {active!r}, expected the Niki default"
    assert colors.primary == NIKI_DARK["primary"]
    assert colors.background == NIKI_DARK["background"]


async def test_building_a_plain_upstream_app_does_not_inherit_the_niki_theme() -> None:
    """Regression: setting Niki's default must not mutate process-wide state.

    An earlier version reassigned `theme.DEFAULT_THEME`, which leaked: a
    `NikiApp` built first left a later `DeepAgentsApp` asking for a theme it had
    never registered. Real apps run one per process and never saw it; the suite
    did, and this is the test that keeps it honest.
    """
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.app import DeepAgentsApp
    from deepagents_code.niki.app import NikiApp

    async def _theme_for(cls: type[DeepAgentsApp]) -> str:
        app = cls(agent=MagicMock(), thread_id="niki-isolation")  # type: ignore[call-arg]
        app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
        async with app.run_test(size=(80, 24)) as pilot:
            await pilot.pause()
            return str(app.theme)

    assert await _theme_for(NikiApp) == "niki"
    assert await _theme_for(DeepAgentsApp) != "niki", (
        "a plain DeepAgentsApp adopted the Niki theme; Niki's default leaked"
    )
