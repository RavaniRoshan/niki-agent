"""Tests for check_lockfiles_pre_commit changed path selection."""

from pathlib import Path
from types import SimpleNamespace

import check_lockfiles_pre_commit
from check_lockfiles_pre_commit import (
    LIBS_ROOT,
    REPO_ROOT,
    _lock_command,
    _lockfile_error,
    _packages_for_paths,
    main,
)

CODE = "libs/code"


def _paths(packages: list[Path]) -> list[str]:
    return [package.relative_to(REPO_ROOT).as_posix() for package in packages]


def test_libs_code_source_selects_the_product_package() -> None:
    """An edit inside the product package selects the product's lockfile."""
    assert _paths(_packages_for_paths([f"{CODE}/niki_agent/app.py"])) == [CODE]
    assert _paths(_packages_for_paths([f"{CODE}/pyproject.toml"])) == [CODE]


def test_libs_code_lockfile_selects_the_product_package() -> None:
    """A direct edit to the product lockfile selects the product package."""
    assert _paths(_packages_for_paths([f"{CODE}/uv.lock"])) == [CODE]


def test_empty_paths_check_all_packages() -> None:
    """No paths preserves full-check behavior for manual runs."""
    packages = _paths(_packages_for_paths([]))
    assert packages == [CODE]


def test_unowned_paths_skip_lock_check() -> None:
    """Non-package edits should not run lock checks in PR mode."""
    assert _packages_for_paths([".github/workflows/ci.yml"]) == []
    assert _packages_for_paths(["docs/niki/ARCHITECTURE.md"]) == []


def test_lock_command_targets_the_product_directory() -> None:
    """The suggested and checked commands point at libs/code."""
    assert _lock_command(LIBS_ROOT / "code", check=True) == [
        "uv",
        "lock",
        "--check",
        "--directory",
        "libs/code",
        "--python",
        "3.12",
    ]


def test_lockfile_error_names_package_and_fix_command() -> None:
    """Failure output points at the stale lockfile and the exact relock command."""
    assert _lockfile_error(LIBS_ROOT / "code") == (
        "::error file=libs/code/uv.lock,title=Out-of-date uv.lock::"
        "libs/code/uv.lock is out of sync with libs/code/pyproject.toml. "
        "From the repository root, run `uv lock --directory libs/code "
        "--python 3.12` and commit the updated lockfile."
    )


def test_main_prints_actionable_error_on_lock_failure(monkeypatch, capsys) -> None:
    """A stale lockfile failure includes the package path and exact fix command."""
    package = LIBS_ROOT / "code"
    commands: list[list[str]] = []

    def fake_run(command: list[str], *, check: bool, cwd: Path) -> SimpleNamespace:
        commands.append(command)
        assert check is False
        assert cwd == REPO_ROOT
        return SimpleNamespace(returncode=1)

    monkeypatch.setattr(check_lockfiles_pre_commit, "package_dirs", lambda: [package])
    monkeypatch.setattr(check_lockfiles_pre_commit.subprocess, "run", fake_run)

    assert main(["libs/code/pyproject.toml"]) == 1
    captured = capsys.readouterr()

    assert commands == [
        ["uv", "lock", "--check", "--directory", "libs/code", "--python", "3.12"]
    ]
    assert "🔍 Checking code" in captured.out
    assert "libs/code/uv.lock is out of sync" in captured.err
    assert "uv lock --directory libs/code --python 3.12" in captured.err
