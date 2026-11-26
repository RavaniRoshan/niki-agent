"""Checksum verification in the release installer (`install/install.sh`).

The release installer downloads an archive and unpacks it. `SEC-001` was that
the archive was trusted without checking the release's `SHA256SUMS`. These tests
pin the verification and its ordering against the shipped script, not a copy of
it.
"""

from __future__ import annotations

import hashlib
import re
import subprocess
from pathlib import Path

INSTALLER = Path(__file__).parents[4] / "install" / "install.sh"


def _extract_shell_function(name: str) -> str:
    """Return the source of a top-level `name() { ... }` block from the script."""
    text = INSTALLER.read_text(encoding="utf-8")
    match = re.search(
        rf"^{re.escape(name)}\(\) \{{.*?^\}}", text, re.MULTILINE | re.DOTALL
    )
    if match is None:
        msg = f"could not locate shell function {name!r} in {INSTALLER}"
        raise AssertionError(msg)
    return match.group(0)


def _run(body: str, tmp_path: Path) -> subprocess.CompletedProcess[str]:
    harness = tmp_path / "harness.sh"
    harness.write_text(
        "set -euo pipefail\n"
        "log() { :; }\n"
        'die() { printf "error: %s\\n" "$*" >&2; exit 1; }\n'
        f"{_extract_shell_function('sha256_of')}\n"
        f"{_extract_shell_function('verify_sha256')}\n"
        f"{body}\n",
        encoding="utf-8",
    )
    return subprocess.run(
        ["bash", str(harness)],
        check=False,
        capture_output=True,
        text=True,
        stdin=subprocess.DEVNULL,
    )


def _write_asset(tmp_path: Path, content: bytes = b"archive bytes") -> tuple[Path, str]:
    asset = tmp_path / "niki-1.1.1-linux.tar.gz"
    asset.write_bytes(content)
    return asset, hashlib.sha256(content).hexdigest()


def test_matching_checksum_passes(tmp_path: Path) -> None:
    asset, digest = _write_asset(tmp_path)
    sums = tmp_path / "SHA256SUMS"
    sums.write_text(f"{digest}  {asset.name}\n", encoding="utf-8")

    proc = _run(
        f"verify_sha256 {str(asset)!r} {str(sums)!r} {asset.name!r}",
        tmp_path,
    )

    assert proc.returncode == 0, proc.stderr


def test_checksum_mismatch_fails(tmp_path: Path) -> None:
    asset, _digest = _write_asset(tmp_path)
    sums = tmp_path / "SHA256SUMS"
    sums.write_text(f"{'0' * 64}  {asset.name}\n", encoding="utf-8")

    proc = _run(
        f"verify_sha256 {str(asset)!r} {str(sums)!r} {asset.name!r}",
        tmp_path,
    )

    assert proc.returncode != 0
    assert "checksum mismatch" in proc.stderr


def test_missing_entry_fails(tmp_path: Path) -> None:
    asset, _digest = _write_asset(tmp_path)
    sums = tmp_path / "SHA256SUMS"
    sums.write_text(f"{'0' * 64}  some-other-asset.tar.gz\n", encoding="utf-8")

    proc = _run(
        f"verify_sha256 {str(asset)!r} {str(sums)!r} {asset.name!r}",
        tmp_path,
    )

    assert proc.returncode != 0
    assert "no entry" in proc.stderr


def test_download_verifies_before_unpacking() -> None:
    """The archive is checked before `tar` sees it, and a missing sums fails closed."""
    text = INSTALLER.read_text(encoding="utf-8")
    verify_at = text.index('verify_sha256 "$tmp/$archive_name"')
    unpack_at = text.index('tar -xzf "$tmp/$archive_name"')
    assert verify_at < unpack_at
    assert "refusing to install an unverified archive" in text
