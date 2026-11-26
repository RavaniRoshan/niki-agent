"""Brand gate for 1.0.0 independence.

Two checks:

1. The retired distribution name (`deepagents_code`) and environment prefix
   (`DEEPAGENTS_CODE_`) exist only where a migration needs them: the legacy
   constant, the changelog, attribution, and the migration tests. This is the
   independence completion criterion.
2. User-facing chrome -- `--version`, `--help`, the key reference, the default
   agent prompt, and the READMEs -- says Niki Agent, not `dcode` or
   "Deep Agents", except for the SDK credit line.

The vendored SDK and ACP trees are excluded: MIT requires their original
prose to survive, and they are attributed through `PROVENANCE.md`.
"""

from __future__ import annotations

import io
import re
from pathlib import Path

import pytest
from rich.console import Console

from niki_agent.niki import branding, help as niki_help

PACKAGE_ROOT = Path(__file__).resolve().parents[3]
REPO_ROOT = PACKAGE_ROOT.parents[1]

RETIRED_DISTRIBUTION = "deepagents_code"
RETIRED_ENV_PREFIX = "DEEPAGENTS_CODE_"

#: Paths where a retirement needs the old spelling to exist.
RETIRED_NAME_ALLOWLIST = {
    "libs/code/niki_agent/_env_vars.py",  # LEGACY_PREFIX
    "libs/code/niki_agent/model_config.py",  # legacy resolution
    "libs/code/CHANGELOG.md",
    "libs/code/niki_agent/sdk/PROVENANCE.md",
    "libs/code/niki_agent/acp_sdk/PROVENANCE.md",
    "libs/code/tests/unit_tests/test_env_vars.py",
    "libs/code/tests/unit_tests/test_model_config.py",
    "libs/code/tests/unit_tests/niki/test_brand_gate.py",
    "README.md",
    "libs/code/README.md",
    "docs/niki/INDEPENDENCE.md",
}

SCAN_SUFFIXES = {".py", ".md", ".toml", ".sh", ".json", ".yml", ".yaml", ".txt"}


def _tracked_files() -> list[Path]:
    """Every text file in the repository, minus generated and vendored trees.

    Returns:
        Paths relative to the repository root.
    """
    skip = {
        ".git",
        ".venv",
        "__pycache__",
        "node_modules",
        ".benchmarks",
        "openwiki",
        "archive",
    }
    ignored = {"libs/code/niki_agent/sdk", "libs/code/niki_agent/acp_sdk", "docs/"}
    found: list[Path] = []
    for path in REPO_ROOT.rglob("*"):
        relative = path.relative_to(REPO_ROOT)
        if any(part in skip for part in relative.parts):
            continue
        if not path.is_file() or path.suffix not in SCAN_SUFFIXES:
            continue
        if any(str(relative).startswith(prefix) for prefix in ignored):
            continue
        if any(str(relative) == str(prefix) for prefix in ignored):
            continue
        found.append(relative)
    return found


@pytest.mark.parametrize("token", [RETIRED_DISTRIBUTION, RETIRED_ENV_PREFIX])
def test_retired_names_survive_only_where_migration_needs_them(token: str) -> None:
    """No product reference may keep the retired name outside the allowlist."""
    offenders = []
    for relative in _tracked_files():
        if str(relative) in RETIRED_NAME_ALLOWLIST:
            continue
        text = (REPO_ROOT / relative).read_text(encoding="utf-8", errors="replace")
        if token in text:
            offenders.append(str(relative))
    assert not offenders, (
        f"{token!r} still appears in {sorted(offenders)}; move migration-only "
        "references into an allowlisted file"
    )


def test_version_output_is_niki_only() -> None:
    """`--version` must not expose a retired product name."""
    line = branding.version_line()
    assert line.startswith("Niki Agent ")
    assert "Deep Agents" not in line
    assert "dcode" not in line


def _rendered_help() -> str:
    """Render the flag help plus the key reference, as `niki --help` does."""
    from unittest.mock import patch

    from niki_agent.ui import show_help

    buffer = io.StringIO()
    console = Console(file=buffer, highlight=False, width=200)
    with patch("niki_agent.ui.console", console):
        show_help()
    niki_help.print_help(console)
    return buffer.getvalue()


def test_help_chrome_says_niki() -> None:
    """The help screen must not send users to a retired command name."""
    rendered = _rendered_help()
    assert "Niki Agent" in rendered
    assert not re.search(r"\bdcode\b", rendered), rendered
    assert "deepagents_code" not in rendered


def test_default_agent_prompt_does_not_name_a_retired_product() -> None:
    """The built-in system prompt is product chrome."""
    prompt = (PACKAGE_ROOT / "niki_agent" / "default_agent_prompt.md").read_text(
        encoding="utf-8"
    )
    assert "dcode" not in prompt
    assert "deepagents_code" not in prompt


def test_readmes_name_the_product() -> None:
    """Both READMEs are distribution-facing.

    A line documenting the deprecated alias is expected; any other use of the
    retired command name is not.
    """
    for relative in ("README.md", "libs/code/README.md"):
        text = (REPO_ROOT / relative).read_text(encoding="utf-8")
        assert "Niki Agent" in text, relative
        offenders = [
            line
            for line in text.splitlines()
            if re.search(r"\bdcode\b", line)
            and not re.search(
                r"alias|deprecat|migrat|legacy|deepagents-code", line, re.IGNORECASE
            )
        ]
        assert not offenders, f"{relative}: {offenders}"
