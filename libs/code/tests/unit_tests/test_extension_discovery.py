"""Tests for extension source resolution and provenance."""

import logging
from dataclasses import dataclass
from pathlib import Path

import pytest

from niki_agent.extensions import discovery
from niki_agent.extensions.discovery import (
    ENTRY_POINT_GROUP,
    LEGACY_ENTRY_POINT_GROUP,
    discover_extensions,
)
from niki_agent.extensions.registry import SourceScope


@pytest.fixture(autouse=True)
def _experimental(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("NIKI_EXPERIMENTAL", "1")
    monkeypatch.setattr(
        "niki_agent.extensions.discovery.importlib.metadata.entry_points",
        lambda **_: (),
    )


def _extension(directory: Path, name: str) -> Path:
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / name
    path.write_text("async def extension(d):\n    pass\n", encoding="utf-8")
    return path


def test_sources_resolve_in_authority_order(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """User, config, CLI, and trusted project sources retain stable order."""
    user = _extension(tmp_path / "user", "a.py")
    configured = _extension(tmp_path / "configured", "b.py")
    temporary = _extension(tmp_path / "temporary", "c.py")
    project = _extension(tmp_path / "project", "d.py")
    monkeypatch.setattr(
        "niki_agent.extensions.discovery.user_extensions_dir",
        lambda: user.parent,
    )

    result = discover_extensions(
        config_paths=(configured, user),
        cli_paths=(temporary,),
        project_dir=project.parent,
    )

    assert [source.path for source in result.sources] == [
        user.resolve(),
        configured.resolve(),
        temporary.resolve(),
        project.resolve(),
    ]
    assert [source.scope for source in result.sources] == [
        SourceScope.USER,
        SourceScope.USER,
        SourceScope.TEMPORARY,
        SourceScope.PROJECT,
    ]


def test_invalid_explicit_path_is_isolated(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A missing CLI source reports an error without blocking valid files."""
    valid = _extension(tmp_path, "valid.py")
    monkeypatch.setattr(
        "niki_agent.extensions.discovery.user_extensions_dir",
        lambda: tmp_path / "absent-user-dir",
    )

    result = discover_extensions(cli_paths=(tmp_path / "missing.py", valid))

    assert [source.path for source in result.sources] == [valid.resolve()]
    assert len(result.errors) == 1
    assert "missing.py" in result.errors[0]


@dataclass(frozen=True)
class _FakeDistribution:
    """The attributes extension discovery reads off a distribution."""

    name: str
    version: str


@dataclass(frozen=True)
class _FakeEntryPoint:
    """Duck-typed `importlib.metadata.EntryPoint` carrying a distribution."""

    name: str
    value: str
    dist: _FakeDistribution


class TestEntryPointGroups:
    """The entry-point group is namespaced to the product, with a legacy read."""

    def test_group_uses_the_product_namespace(self) -> None:
        assert ENTRY_POINT_GROUP == "niki.extensions"

    def test_legacy_group_still_resolves_with_a_deprecation_log(
        self,
        tmp_path: Path,
        monkeypatch: pytest.MonkeyPatch,
        caplog: pytest.LogCaptureFixture,
    ) -> None:
        """A distribution still on the old group loads, with one INFO line."""
        entry = _FakeEntryPoint(
            name="legacy",
            value="json",
            dist=_FakeDistribution(name="legacy-ext", version="1.0"),
        )
        monkeypatch.setattr(
            discovery.importlib.metadata,
            "entry_points",
            lambda *, group: (entry,) if group == LEGACY_ENTRY_POINT_GROUP else (),
        )
        monkeypatch.setattr(
            discovery, "user_extensions_dir", lambda: tmp_path / "absent-user-dir"
        )

        with caplog.at_level(logging.INFO, logger="niki_agent.extensions.discovery"):
            result = discover_extensions()

        assert [source.source_id for source in result.sources] == ["legacy@entry-point"]
        records = [r for r in caplog.records if r.levelno == logging.INFO]
        assert len(records) == 1
        message = records[0].getMessage()
        assert "legacy-ext" in message
        assert ENTRY_POINT_GROUP in message
