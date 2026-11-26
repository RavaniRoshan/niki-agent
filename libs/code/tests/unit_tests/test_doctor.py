"""Unit tests for the `niki doctor` command."""

import argparse
import io
import json
import sys
import time
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import pytest
from rich.console import Console

from niki_agent.doctor import (
    DiagnosticItem,
    DiagnosticSection,
    _build_commit,
    _commit_hash,
    collect_sections,
    run_doctor_command,
)
from niki_agent.extras_info import collect_version_report


class TestDoctorArgs:
    """Tests for `doctor` argument parsing."""


class TestDiagnosticSection:
    """Tests for the section dataclass health aggregation."""


class TestCollectSections:
    """Tests for the diagnostic data collection."""

    def test_diagnostics_reports_version(self) -> None:
        """The Diagnostics section reports the running CLI version."""
        from niki_agent._version import __version__

        # Isolate the SDK requirement check so the workspace pin cannot inject a
        # mismatch annotation into the CLI value under test. Editable installs
        # resolve the pin through `_sdk_requirement_for_cli`.
        with patch(
            "niki_agent.extras_info._sdk_requirement_for_cli",
            return_value=None,
        ):
            diagnostics = collect_sections()[0]
        labels = {item.label: item.value for item in diagnostics.items}
        assert labels["niki-agent"] == __version__
        assert "Commit hash" in labels
        assert labels["Commit hash"]
        assert "Platform" in labels
        assert "Install method" in labels


class TestDiagnosticsVersionReport:
    """The vendored SDK has no install-time diagnostics."""

    def test_bundled_sdk_reports_its_version(self) -> None:
        report = collect_version_report()
        assert report.sdk.status == "resolved"
        assert report.sdk.metadata_version
        assert report.sdk_requirement is None


class TestCollectTracing:
    """Tests for the Tracing diagnostic section."""

    def _section(self, **kwargs: object) -> DiagnosticSection:
        from niki_agent.config import TracingStatus
        from niki_agent.doctor import _collect_tracing

        defaults: dict[str, object] = {
            "enabled": False,
            "explicitly_disabled": False,
            "has_credentials": False,
            "endpoint": None,
            "project": None,
            "project_is_default": False,
            "replica_project": None,
        }
        defaults.update(kwargs)
        # `defaults` is intentionally heterogeneous (`dict[str, object]`), so the
        # unpack can't be statically matched to each field's type.
        status = TracingStatus(**defaults)  # ty: ignore[invalid-argument-type]
        with patch("niki_agent.config.get_tracing_status", return_value=status):
            return _collect_tracing()

    def test_not_configured_is_healthy(self) -> None:
        """An unconfigured, keyless setup is informational, not a failure."""
        section = self._section(enabled=False, project="niki-agent")
        assert section.title == "Tracing"
        assert section.ok is True
        labels = {item.label: item.value for item in section.items}
        assert labels["Tracing"] == "not configured"
        assert labels["Credentials"] == "not set"
        assert labels["Project"] == "niki-agent"

    def test_explicitly_disabled_reads_disabled(self) -> None:
        """An explicit opt-out reads `disabled`, not `not configured`."""
        section = self._section(enabled=False, explicitly_disabled=True)
        assert section.ok is True
        labels = {item.label: item.value for item in section.items}
        assert labels["Tracing"] == "disabled"
        assert labels["Credentials"] == "not set"

    def test_enabled_without_credentials_is_unhealthy(self) -> None:
        """Tracing on with no key and no endpoint is a genuine problem."""
        section = self._section(enabled=True, has_credentials=False)
        assert section.ok is False
        creds = next(i for i in section.items if i.label == "Credentials")
        assert creds.ok is False

    def test_enabled_with_credentials_is_healthy(self) -> None:
        """A configured key keeps the section healthy and reports the project."""
        section = self._section(enabled=True, has_credentials=True, project="my-proj")
        assert section.ok is True
        labels = {item.label: item.value for item in section.items}
        assert labels["Tracing"] == "enabled"
        assert labels["Credentials"] == "configured"
        assert labels["Project"] == "my-proj"

    def test_keyless_custom_endpoint_is_healthy(self) -> None:
        """A custom endpoint is a valid keyless setup, so it stays healthy."""
        section = self._section(
            enabled=True,
            has_credentials=False,
            endpoint="http://localhost:1984",
        )
        assert section.ok is True
        labels = {item.label: item.value for item in section.items}
        assert labels["Endpoint"] == "http://localhost:1984"

    def test_endpoint_is_sanitized(self) -> None:
        """Endpoint diagnostics redact userinfo, path, query, and fragment."""
        section = self._section(
            enabled=True,
            has_credentials=False,
            endpoint=(
                "https://user:secret@example.com:8443/trace?api_key=secret-token#frag"
            ),
        )
        labels = {item.label: item.value for item in section.items}
        assert labels["Endpoint"] == "https://example.com:8443"
        assert "secret" not in labels["Endpoint"]
        assert "api_key" not in labels["Endpoint"]

    def test_gateway_yes_for_langsmith_host(self) -> None:
        """A `smith.langchain.com` endpoint routes through the managed gateway."""
        section = self._section(
            enabled=True,
            has_credentials=True,
            endpoint="https://api.smith.langchain.com",
        )
        labels = {item.label: item.value for item in section.items}
        assert labels["Gateway"] == "yes"
        # A configured endpoint surfaces both items; they must not interfere.
        assert "Endpoint" in labels

    def test_gateway_no_for_custom_endpoint(self) -> None:
        """A self-hosted/dev endpoint is not the LangSmith managed gateway."""
        section = self._section(
            enabled=True,
            has_credentials=False,
            endpoint="http://localhost:1984",
        )
        labels = {item.label: item.value for item in section.items}
        assert labels["Gateway"] == "no"

    def test_gateway_unknown_for_unparseable_endpoint(self) -> None:
        """An endpoint with no parseable host reports `unknown`, never `no`."""
        section = self._section(
            enabled=True,
            has_credentials=True,
            endpoint="not a url",
        )
        labels = {item.label: item.value for item in section.items}
        assert labels["Gateway"] == "unknown"

    def test_gateway_no_when_replica_endpoint_is_self_hosted(self) -> None:
        """A self-hosted replica target counts even with no primary endpoint."""
        section = self._section(
            enabled=True,
            has_credentials=True,
            endpoint=None,
            runs_endpoints=("http://localhost:1984",),
        )
        labels = {item.label: item.value for item in section.items}
        assert labels["Gateway"] == "no"

    def test_gateway_yes_when_replica_endpoint_is_gateway(self) -> None:
        """Replica targets on the managed gateway keep the report `yes`."""
        section = self._section(
            enabled=True,
            has_credentials=True,
            endpoint=None,
            runs_endpoints=("https://eu.api.smith.langchain.com",),
        )
        labels = {item.label: item.value for item in section.items}
        assert labels["Gateway"] == "yes"

    def test_replica_project_listed_when_set(self) -> None:
        """A configured replica project is surfaced as its own item."""
        section = self._section(
            enabled=True,
            has_credentials=True,
            replica_project="replica",
        )
        labels = {item.label: item.value for item in section.items}
        assert labels["Replica project"] == "replica"


class TestEndpointGatewayState:
    """Tests for the single-endpoint tracing gateway-host classifier."""

    def test_suffix_lookalike_host_is_not_gateway(self) -> None:
        """A host containing the domain as a substring is not the gateway."""
        from niki_agent.doctor import _endpoint_gateway_state

        assert (
            _endpoint_gateway_state("https://smith.langchain.com.evil.example") == "no"
        )


class TestCollectUpdates:
    """Tests for the Updates diagnostic section."""

    def _labels(
        self,
        cache_file: Path,
        *,
        editable: bool = False,
        checks_enabled: bool = True,
        cached: tuple[bool, str | None] = (False, "1.0.0"),
    ) -> dict[str, str]:
        """Collect the Updates labels, reading `checked_at` from `cache_file`.

        Patches `CACHE_FILE` rather than `get_last_update_check_time` so the
        section flows through the genuine reader and the epoch -> ISO ->
        relative-time conversion, not a stub.
        """
        from niki_agent.doctor import _collect_updates

        with (
            patch(
                "niki_agent.config._is_editable_install",
                return_value=editable,
            ),
            patch(
                "niki_agent.update_check.is_update_check_enabled",
                return_value=checks_enabled,
            ),
            patch(
                "niki_agent.update_check.is_auto_update_enabled",
                return_value=True,
            ),
            patch(
                "niki_agent.update_check.get_cached_update_available",
                return_value=cached,
            ),
            patch("niki_agent.update_check.CACHE_FILE", cache_file),
        ):
            section = _collect_updates()
        return {item.label: item.value for item in section.items}

    def _stale_cache(self, tmp_path: Path) -> Path:
        """Write a cache stamped three days ago, well past `CACHE_TTL`."""
        cache = tmp_path / "latest_version.json"
        cache.write_text(
            json.dumps({"checked_at": time.time() - 3 * 86_400}), encoding="utf-8"
        )
        return cache

    def _fresh_cache(self, tmp_path: Path) -> Path:
        """Write a cache stamped five minutes ago, well inside `CACHE_TTL`."""
        cache = tmp_path / "latest_version.json"
        cache.write_text(
            json.dumps({"checked_at": time.time() - 300}), encoding="utf-8"
        )
        return cache

    def test_last_checked_just_now_on_future_stamp(self, tmp_path: Path) -> None:
        """A future stamp (clock skew) renders as `just now`, not a crash."""
        cache = tmp_path / "latest_version.json"
        cache.write_text(
            json.dumps({"checked_at": time.time() + 3600}), encoding="utf-8"
        )
        assert self._labels(cache)["Last checked"] == "just now"

    def test_last_checked_never_on_corrupt_stamp(self, tmp_path: Path) -> None:
        """A non-finite stamp fails soft to `never` instead of crashing doctor."""
        cache = tmp_path / "latest_version.json"
        cache.write_text(json.dumps({"checked_at": float("nan")}), encoding="utf-8")
        assert self._labels(cache)["Last checked"] == "never"

    def test_latest_version_reports_cached_answer(self, tmp_path: Path) -> None:
        """A cached answer is reported even though it is older than the TTL."""
        cache = self._stale_cache(tmp_path)
        assert self._labels(cache)["Latest version"] == "up to date"
        available = self._labels(cache, cached=(True, "9.9.9"))
        assert available["Latest version"] == "v9.9.9 available"

    def test_latest_version_blames_editable_install(self, tmp_path: Path) -> None:
        """Editable installs never check, so the row names that as the cause."""
        labels = self._labels(
            self._stale_cache(tmp_path), editable=True, cached=(False, None)
        )
        assert labels["Latest version"] == "not checked (editable install)"
        assert labels["Auto-updates"] == "disabled (editable install)"
        assert labels["Last checked"] == "3d ago"

    def test_latest_version_blames_disabled_checks(self, tmp_path: Path) -> None:
        """Disabled checks freeze the cache, so the row names that as the cause."""
        labels = self._labels(
            self._stale_cache(tmp_path), checks_enabled=False, cached=(False, None)
        )
        assert labels["Latest version"] == "not checked (checks disabled)"
        assert labels["Update checks"] == "disabled"

    def test_latest_version_reports_stale_cache(self, tmp_path: Path) -> None:
        """An enabled checker with a rejected cache reads as stale, not unknown."""
        labels = self._labels(self._stale_cache(tmp_path), cached=(False, None))
        assert labels["Latest version"] == "unknown (cache stale)"
        assert labels["Last checked"] == "3d ago"

    def test_latest_version_reports_incomplete_cache(self, tmp_path: Path) -> None:
        """A current cache with no usable entry is incomplete, not stale.

        Reachable when only pre-release pins were written or a pre-release
        install meets a stable-only payload, so the row must not claim the cache
        expired.
        """
        labels = self._labels(self._fresh_cache(tmp_path), cached=(False, None))
        assert labels["Latest version"] == "unknown (cache incomplete)"
        assert labels["Last checked"] == "5m ago"

    def test_latest_version_reports_never_checked(self, tmp_path: Path) -> None:
        """With no stamp on disk, no check has ever completed."""
        labels = self._labels(tmp_path / "latest_version.json", cached=(False, None))
        assert labels["Latest version"] == "unknown (never checked)"
        assert labels["Last checked"] == "never"


class TestCommitHash:
    """Tests for git commit hash detection."""

    def test_baked_commit_preferred_over_git(self) -> None:
        """A build-stamped commit wins for a wheel and skips the live git probe."""
        with (
            patch("niki_agent.doctor._build_commit", return_value="deadbee"),
            patch("niki_agent.config._is_editable_install", return_value=False),
            patch("shutil.which") as which,
            patch("subprocess.run") as run,
        ):
            assert _commit_hash("/tmp") == "deadbee"

        which.assert_not_called()
        run.assert_not_called()

    def test_editable_install_ignores_baked_commit(self) -> None:
        """An editable install ignores a (possibly stale) stamp and probes git."""
        with (
            patch("niki_agent.doctor._build_commit", return_value="deadbee"),
            patch("niki_agent.config._is_editable_install", return_value=True),
            patch("shutil.which", return_value=None),
            patch("subprocess.run") as run,
        ):
            assert _commit_hash("/tmp") == "unknown"

        run.assert_not_called()

    def test_build_commit_missing_module(self) -> None:
        """No generated module (editable/dev install) yields `None`."""
        with patch.dict(sys.modules, {"niki_agent._build_info": None}):
            assert _build_commit() is None

    def test_build_commit_reads_stamped_value(self) -> None:
        """A generated module exposes its stamped commit."""
        stub = SimpleNamespace(BUILD_COMMIT="abc1234")
        with patch.dict(sys.modules, {"niki_agent._build_info": stub}):
            assert _build_commit() == "abc1234"

    @pytest.mark.parametrize("value", ["", "   ", None])
    def test_build_commit_blank_value_is_none(self, value: str | None) -> None:
        """A present module with a blank stamp yields `None` (falls back to git)."""
        stub = SimpleNamespace(BUILD_COMMIT=value)
        with patch.dict(sys.modules, {"niki_agent._build_info": stub}):
            assert _build_commit() is None

    def test_build_commit_corrupt_module_returns_none(self) -> None:
        """A present-but-corrupt stamp degrades to `None` instead of crashing."""

        class _Corrupt:
            def __getattr__(self, name: str) -> str:
                msg = "corrupt stamp"
                raise ValueError(msg)

        with patch.dict(sys.modules, {"niki_agent._build_info": _Corrupt()}):
            assert _build_commit() is None


class TestConfigurationSection:
    """The Configuration section is the only report of which paths are live.

    `managed_tools` and the update-lock warnings both tell the user to run
    `dcode doctor` to find out which of two locations is in use, so a missing
    row leaves that question unanswerable.
    """

    def _labels(self) -> list[str]:
        from niki_agent.doctor import _collect_configuration

        return [item.label for item in _collect_configuration().items]

    def test_reports_both_shared_locations(self) -> None:
        """Managed binaries and the installation lock directory always show."""
        labels = self._labels()

        assert "Managed binaries" in labels
        assert "Update locks" in labels

    def test_reports_a_skipped_home_check_as_a_problem(self) -> None:
        """A security check that stopped running must not be silent."""
        from dataclasses import replace

        from niki_agent._paths import PATHS
        from niki_agent.doctor import _collect_configuration

        with patch("niki_agent._paths.PATHS", replace(PATHS, home_check_skipped=True)):
            items = {item.label: item for item in _collect_configuration().items}

        assert "Profile safety check" in items
        assert items["Profile safety check"].ok is False


class TestRunDoctorCommand:
    """Tests for the text and JSON rendering paths."""

    def _run_text(self, *, force_terminal: bool = False) -> tuple[int, str]:
        buf = io.StringIO()
        test_console = Console(
            file=buf, force_terminal=force_terminal, highlight=False, width=200
        )
        args = argparse.Namespace(output_format="text")
        with (
            patch("niki_agent.config.console", test_console),
            patch("niki_agent.doctor.invoked_name", return_value="niki"),
        ):
            code = run_doctor_command(args)
        return code, buf.getvalue()

    def test_text_output_contains_sections(self) -> None:
        """Text output renders each section title and key facts."""
        # Isolate the SDK requirement check so a workspace where the declared
        # `deepagents` pin intentionally leads or lags the installed SDK does
        # not make the section unhealthy; the mismatch path has dedicated
        # coverage. Editable installs resolve via `_sdk_requirement_for_cli`.
        with patch(
            "niki_agent.extras_info._sdk_requirement_for_cli",
            return_value=None,
        ):
            code, output = self._run_text()
        assert code == 0
        assert "Diagnostics" in output
        assert "Updates" in output
        assert "Tracing" in output
        assert "Configuration" in output
        assert "niki-agent" in output
        # The tips name the command the process was launched with, not a fixed
        # one: `dcode` remains a deprecated alias and shims are common.
        assert "niki config get <key>" in output
        assert "niki --version" in output
        assert "niki -v" in output
        assert "dcode" not in output

    def test_commit_hash_renders_as_link(self) -> None:
        """Text output links the hash to GitHub."""
        with patch("niki_agent.doctor._commit_hash", return_value="abc1234"):
            _, output = self._run_text(force_terminal=True)

        assert "https://github.com/RavaniRoshan/niki-agent/commit/abc1234" in output

    def test_invalid_commit_hash_renders_without_link(self) -> None:
        """Text output leaves an invalid hash unlinked."""
        with patch("niki_agent.doctor._commit_hash", return_value="not-a-sha"):
            _, output = self._run_text(force_terminal=True)

        assert "not-a-sha" in output
        assert "https://github.com/RavaniRoshan/niki-agent/commit/" not in output


class TestPathStatus:
    """Tests for the path-existence diagnostic item."""

    def test_unreadable_path_is_unhealthy(self, monkeypatch) -> None:
        """An unreadable path is flagged as a genuine problem (`ok=False`)."""
        from pathlib import Path

        from niki_agent.doctor import _path_status

        def _raise(self: Path) -> object:  # noqa: ARG001  # must match Path.stat signature
            msg = "permission denied"
            raise PermissionError(msg)

        monkeypatch.setattr(Path, "stat", _raise)
        item = _path_status("Config file", "/some/protected/path")
        assert item.ok is False
        assert "unreadable" in item.value


class TestDoctorHelp:
    """Tests for the doctor help screen."""


class TestFallbackLocationReporting:
    """`doctor` must not call an unusable primary location healthy.

    `classify_path` answers "is it there". A present but root-owned managed-bin
    directory is exactly the condition `FALLBACK_BIN_DIR` exists for, and
    reporting it as `exists` sends the user looking somewhere else.
    """

    def test_a_missing_directory_is_not_a_permission_problem(
        self, tmp_path: Path
    ) -> None:
        """Lazily created directories must not be reported as unwritable."""
        from niki_agent.doctor import _writable_path_status

        item = _writable_path_status("Managed binaries", tmp_path / "absent")

        assert item.ok is True
        assert "not created" in item.value
