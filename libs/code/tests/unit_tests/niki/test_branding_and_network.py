"""V2 (branding) and L6 (outbound network) as executable checks.

V2 is a naming discipline, so it is enforced two ways: Niki's package source
must not spell a retired upstream name in a string literal, and the committed
first-run frame must not show one either.

L6 is a policy, so it is enforced by asserting the defaults and the owner
opt-outs.
"""

from __future__ import annotations

import ast
import re
from pathlib import Path
from typing import TYPE_CHECKING

import pytest

from niki_agent.niki import branding, network

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


def test_version_line_names_niki() -> None:
    """`--version` must say Niki Agent and nothing else."""
    line = branding.version_line()
    assert line.startswith("Niki Agent "), line
    assert branding.VERSION_LINE_SUFFIX == ""
    assert re.match(r"^Niki Agent \d+\.\d+\.\d+", line), f"no version in {line!r}"


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


def test_niki_is_the_primary_script_and_aliases_still_work() -> None:
    """`niki` is primary; `dcode` and `deepagents-code` stay as deprecated aliases."""
    import tomllib

    pyproject = PACKAGE_ROOT / "pyproject.toml"
    scripts = tomllib.loads(pyproject.read_text(encoding="utf-8"))["project"]["scripts"]

    assert scripts["niki"] == "niki_agent.niki.entry:niki_main"
    assert scripts["dcode"] == "niki_agent:cli_main"
    assert scripts["deepagents-code"] == "niki_agent:cli_main"


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
    """L6: Niki's network features are on by default, with an owner opt-out.

    The owner chose parity with what users expect from a terminal coding agent:
    update checks, self-update, and remote managed config all work unless an
    owner disables them by name.
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
    monkeypatch.setenv("NIKI_NO_UPDATE_CHECK", "0")

    network.apply_niki_network_policy()

    import os

    assert os.environ["NIKI_NO_UPDATE_CHECK"] == "0", (
        "the policy overwrote an explicit owner setting; it must only supply defaults"
    )


def test_the_install_script_is_nikis_own() -> None:
    """The promoted install command must fetch Niki's installer, not a fork's."""
    command = network.install_script_command()

    assert "RavaniRoshan/niki-agent" in command, command
    assert "langch.in/dcode" not in command, command
    assert "install.sh" in command, command


def test_snapshot_frames_do_not_show_the_upstream_product_name() -> None:
    """V2: the rendered first-run frame must not show 'Deep Agents'.

    The keymap body legitimately credits upstream ("built on Deep Agents"), the
    one attribution `branding.py` requires rather than forbids. The credit
    wraps at narrow widths, so the frame's text is joined, its whitespace
    collapsed, and the whole credit phrase stripped; a real leak still fails.
    """
    frames = sorted(SNAPSHOT_DIR.rglob("*.raw"))
    assert frames, (
        "no committed snapshots; run the snapshot suite with --snapshot-update"
    )

    def _plain(node: str) -> str:
        # SVG stores spaces as non-breaking entities.
        return node.replace("&#160;", " ").replace("&gt;", ">").replace("&amp;", "&")

    def _collapse(text: str) -> str:
        return re.sub(r"\s+", " ", text.replace("\u00a0", " ")).strip()

    offenders = []
    for frame in frames:
        text = frame.read_text(encoding="utf-8", errors="replace")
        nodes = [_plain(node) for node in re.findall(r">([^<>]{2,})</text>", text)]
        # Join before stripping: a narrow frame wraps the credit across text
        # nodes, so a per-node strip cannot match it.
        remainder = _collapse(" ".join(nodes))
        for credit in (branding.VERSION_LINE_SUFFIX, branding.SDK_CREDIT):
            remainder = remainder.replace(_collapse(credit.removeprefix(", ")), "")
        if "Deep Agents" in remainder:
            offenders.append(f"{frame.name}: {remainder!r}")
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

    from niki_agent.niki.app import NikiApp
    from niki_agent.niki.theme import NIKI_DARK
    from niki_agent.theme import get_theme_colors

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

    from niki_agent.app import DeepAgentsApp
    from niki_agent.niki.app import NikiApp

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
