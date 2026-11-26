"""Tests for optional-dependency status inspection."""

import tomllib
from collections.abc import Iterator
from importlib.metadata import PackageNotFoundError
from pathlib import Path
from unittest.mock import MagicMock, PropertyMock, patch

import pytest
from packaging.requirements import Requirement
from packaging.utils import canonicalize_name

from niki_agent.extras_info import (
    _COMPOSITE_EXTRAS,
    BASE_DEPENDENCY_EXTRAS,
    COMPOSITE_EXTRA_MEMBERS,
    KNOWN_EXTRAS,
    MODEL_PROVIDER_EXTRAS,
    SANDBOX_EXTRAS,
    STANDALONE_EXTRAS,
    DistributionMetadataStatus,
    DistributionVersion,
    VersionReport,
    _display_sdk_version,
    _editable_sdk_is_cli_workspace_sibling,
    _editable_sdk_source_root,
    _requirement_satisfied,
    _resolve_source_path,
    _sdk_requirement_comparison_version,
    collect_cli_version_info,
    collect_sdk_version_info,
    collect_version_report,
    composite_extras_providing,
    format_cli_version_annotation,
    format_known_extras,
    format_sdk_version_annotation,
    get_extras_status,
    resolve_sdk_version,
    sdk_requirement_from_cli,
)

_PYPROJECT_PATH = Path(__file__).resolve().parents[2] / "pyproject.toml"


@pytest.fixture
def sdk_root(tmp_path: Path) -> Path:
    """A source root laid out like an editable `deepagents` checkout."""
    root = tmp_path / "workspace" / "libs" / "deepagents"
    (root / "deepagents").mkdir(parents=True)
    return root


@pytest.fixture(autouse=True)
def _running_sdk_from(sdk_root: Path) -> Iterator[None]:
    """Place the locatable `deepagents` package inside `sdk_root` for every test.

    `_editable_sdk_source_root` only credits an editable record whose source root
    contains the package `find_spec` would locate, so the two have to agree about
    where that package lives. Tests that need them to disagree patch this
    themselves.
    """
    with patch(
        "niki_agent.extras_info._running_sdk_package_root",
        return_value=(sdk_root / "deepagents").resolve(),
    ):
        yield


def _sdk_dist(*, raw: str | None = None) -> MagicMock:
    """Build a minimal `importlib.metadata.Distribution` stand-in.

    `raw` is the exact `direct_url.json` text the stand-in serves; `None` serves
    no file, as a wheel or a source-tree `*.egg-info` would.
    """
    dist = MagicMock()
    dist.name = "deepagents"
    dist.metadata = {"Name": "deepagents"}
    dist.read_text.return_value = raw
    return dist


def _write_cli_pyproject(root: Path, requirement: str) -> None:
    """Write the minimal editable dcode project metadata used by version tests."""
    root.mkdir(parents=True, exist_ok=True)
    (root / "pyproject.toml").write_text(
        f'[project]\nname = "niki-agent"\ndependencies = ["{requirement}"]\n',
        encoding="utf-8",
    )


def _optional_dependencies() -> dict[str, list[str]]:
    """Return optional dependencies declared in `pyproject.toml`."""
    data = tomllib.loads(_PYPROJECT_PATH.read_text(encoding="utf-8"))
    return data["project"]["optional-dependencies"]


def _declared_extras() -> frozenset[str]:
    """Return non-composite extras declared in `pyproject.toml`."""
    return frozenset(_optional_dependencies()) - _COMPOSITE_EXTRAS


def test_incomplete_extras_are_omitted() -> None:
    mock_dist = MagicMock()
    mock_dist.requires = [
        "langchain-anthropic>=1.0.0 ; extra == 'anthropic'",
        "fake-absent-package>=1.0.0 ; extra == 'custom'",
        "aiohttp>=3.14.3 ; extra == 'nvidia'",
        "langchain-nvidia-ai-endpoints>=1.4.3 ; extra == 'nvidia'",
    ]

    def fake_version(name: str) -> str:
        if name == "langchain-anthropic":
            return "1.4.0"
        if name == "aiohttp":
            return "3.14.3"
        raise PackageNotFoundError(name)

    with (
        patch("niki_agent.extras_info.distribution", return_value=mock_dist),
        patch("niki_agent.extras_info.pkg_version", side_effect=fake_version),
    ):
        extras = get_extras_status()

    assert extras == {"anthropic": [("langchain-anthropic", "1.4.0")]}


def test_complete_multi_package_extras_are_kept() -> None:
    mock_dist = MagicMock()
    mock_dist.requires = [
        "aiohttp>=3.14.3 ; extra == 'nvidia'",
        "langchain-nvidia-ai-endpoints>=1.4.3 ; extra == 'nvidia'",
    ]

    def fake_version(name: str) -> str:
        if name == "aiohttp":
            return "3.14.3"
        if name == "langchain-nvidia-ai-endpoints":
            return "1.4.3"
        raise PackageNotFoundError(name)

    with (
        patch("niki_agent.extras_info.distribution", return_value=mock_dist),
        patch("niki_agent.extras_info.pkg_version", side_effect=fake_version),
    ):
        extras = get_extras_status()

    assert extras == {
        "nvidia": [
            ("aiohttp", "3.14.3"),
            ("langchain-nvidia-ai-endpoints", "1.4.3"),
        ],
    }


def test_skips_known_composite_extras() -> None:
    # The build backend flattens composite extras like `all-providers`
    # into their component packages, so name-based filtering is needed to
    # avoid duplicating the full list in the output.
    mock_dist = MagicMock()
    mock_dist.requires = [
        "langchain-anthropic>=1.0.0 ; extra == 'all-providers'",
        "langchain-baseten>=1.0.0 ; extra == 'all-providers'",
        "langchain-daytona>=1.0.0 ; extra == 'all-sandboxes'",
        "langchain-vercel-sandbox>=0.0.1 ; extra == 'all-sandboxes'",
        "langchain-anthropic>=1.0.0 ; extra == 'anthropic'",
    ]

    with (
        patch("niki_agent.extras_info.distribution", return_value=mock_dist),
        patch("niki_agent.extras_info.pkg_version", return_value="1.0.0"),
    ):
        extras = get_extras_status()

    assert "all-providers" not in extras
    assert "all-sandboxes" not in extras
    assert extras["anthropic"] == [("langchain-anthropic", "1.0.0")]


def test_extras_taxonomy_covers_pyproject() -> None:
    """Every declared extra must be classified in one of the taxonomy sets.

    A new extra added to `pyproject.toml` without an entry in
    `MODEL_PROVIDER_EXTRAS`, `SANDBOX_EXTRAS`, or `STANDALONE_EXTRAS` would
    silently fall out of the onboarding dependency screen. This drift test
    forces the contributor to update one of those constants alongside the
    dependency.
    """
    declared = _declared_extras()
    classified = MODEL_PROVIDER_EXTRAS | SANDBOX_EXTRAS | STANDALONE_EXTRAS

    uncategorized = declared - classified
    assert not uncategorized, (
        f"pyproject.toml declares extras not classified in extras_info: "
        f"{sorted(uncategorized)}"
    )

    stale = classified - declared
    assert not stale, (
        f"extras_info classifies extras not declared in pyproject.toml: {sorted(stale)}"
    )


def test_extras_categories_are_disjoint() -> None:
    """An extra can only be classified in one taxonomy set."""
    pairs = (
        ("providers/sandboxes", MODEL_PROVIDER_EXTRAS & SANDBOX_EXTRAS),
        ("providers/standalone", MODEL_PROVIDER_EXTRAS & STANDALONE_EXTRAS),
        ("sandboxes/standalone", SANDBOX_EXTRAS & STANDALONE_EXTRAS),
    )
    for label, overlap in pairs:
        assert not overlap, f"Extras classified twice in {label}: {sorted(overlap)}"


def _parse_known_extras(rendered: str) -> dict[str, list[str]]:
    """Parse `format_known_extras` output into `{label: [extras]}`.

    Lets tests assert per-line grouping and ordering rather than matching
    substrings against the whole blob, which would pass even if extras were
    rendered under the wrong category or all collapsed onto one line.
    """
    groups: dict[str, list[str]] = {}
    for line in rendered.splitlines()[1:]:  # skip the "Available extras:" header
        label, _, extras = line.strip().partition(": ")
        groups[label] = extras.split(", ")
    return groups


def test_format_known_extras_lists_exactly_known_extras() -> None:
    """The listing must contain every `KNOWN_EXTRAS` member and nothing else."""
    rendered = format_known_extras()
    assert rendered.startswith("Available extras:")
    groups = _parse_known_extras(rendered)
    rendered_extras = {extra for extras in groups.values() for extra in extras}
    # Bidirectional: catches both a new category left out of the listing and a
    # listing that drifts ahead of `KNOWN_EXTRAS`.
    assert rendered_extras == set(KNOWN_EXTRAS)


def test_format_known_extras_groups_extras_under_correct_label() -> None:
    """Each category renders under its own label with alphabetical ordering."""
    groups = _parse_known_extras(format_known_extras())
    assert groups["Model providers"] == sorted(MODEL_PROVIDER_EXTRAS)
    assert groups["Sandboxes"] == sorted(SANDBOX_EXTRAS)
    assert groups["Other"] == sorted(STANDALONE_EXTRAS)


# `verify_interpreter_deps` does a lazy `from niki_agent.config import
# _is_editable_install` each call, so the symbol is resolved against
# `niki_agent.config` at call time. Patch the source module — patching
# `niki_agent.extras_info._is_editable_install` would not work (it isn't
# bound there as a module-level attribute).


class TestResolveSdkVersion:
    """The SDK is vendored, so its version comes from the copy itself."""

    def test_resolves_the_vendored_sdk_version(self) -> None:
        """A vendored SDK always resolves, with no external distribution."""
        from niki_agent.sdk._version import __version__ as vendored

        version, status = resolve_sdk_version()
        assert (version, status) == (vendored, "resolved")

    def test_does_not_look_up_an_external_sdk_distribution(self) -> None:
        """No `deepagents` distribution lookup happens for the vendored copy."""
        with patch("niki_agent.extras_info.pkg_version") as mock:
            resolve_sdk_version()
        assert all(call.args != ("deepagents",) for call in mock.call_args_list)


def _distribution_version(
    *,
    name: str = "deepagents",
    source_version: str | None = None,
    metadata_version: str | None = None,
    editable: bool = False,
    source_path: str | None = None,
    status: DistributionMetadataStatus = "resolved",
) -> DistributionVersion:
    """Build a `DistributionVersion` with sensible defaults for tests."""
    return DistributionVersion(
        name=name,
        source_version=source_version,
        metadata_version=metadata_version,
        editable=editable,
        source_path=source_path,
        status=status,
    )


class TestContractHome:
    """Tests for `~`-contraction of displayed editable source paths."""

    def test_contracts_home_prefix(self) -> None:
        """A path under the home directory is shown with a `~` prefix."""
        from niki_agent.extras_info import _contract_home

        with patch("niki_agent.extras_info.Path.home", return_value=Path("/home/dev")):
            assert _contract_home(Path("/home/dev/src/proj")) == "~/src/proj"

    def test_exact_home_becomes_tilde(self) -> None:
        """The home directory itself contracts to a bare `~`."""
        from niki_agent.extras_info import _contract_home

        with patch("niki_agent.extras_info.Path.home", return_value=Path("/home/dev")):
            assert _contract_home(Path("/home/dev")) == "~"

    def test_non_home_path_is_unchanged(self) -> None:
        """A path outside the home directory is returned verbatim."""
        from niki_agent.extras_info import _contract_home

        with patch("niki_agent.extras_info.Path.home", return_value=Path("/home/dev")):
            assert _contract_home(Path("/opt/other")) == "/opt/other"

    def test_unresolvable_home_returns_raw_path(self) -> None:
        """When the home directory cannot be determined, the raw path is kept."""
        from niki_agent.extras_info import _contract_home

        with patch("niki_agent.extras_info.Path.home", side_effect=RuntimeError):
            assert _contract_home(Path("/home/dev/x")) == "/home/dev/x"


class TestDistributionVersion:
    """Tests for the structured single-distribution version representation."""

    def test_primary_prefers_source_for_editable(self) -> None:
        """Editable installs report the live source version as primary."""
        info = _distribution_version(
            source_version="1.2.4", metadata_version="1.2.3", editable=True
        )
        assert info.primary_version == "1.2.4"

    def test_primary_uses_metadata_for_non_editable(self) -> None:
        """Non-editable installs report the metadata version as primary."""
        info = _distribution_version(
            source_version="9.9.9", metadata_version="1.2.3", editable=False
        )
        assert info.primary_version == "1.2.3"

    def test_primary_falls_back_to_metadata_without_source(self) -> None:
        """An editable install with no readable source version uses metadata."""
        info = _distribution_version(
            source_version=None, metadata_version="1.2.3", editable=True
        )
        assert info.primary_version == "1.2.3"

    def test_has_drift_true_when_versions_differ(self) -> None:
        """Drift is reported when both versions are known and differ."""
        info = _distribution_version(source_version="1.2.4", metadata_version="1.2.3")
        assert info.has_drift is True

    def test_has_drift_false_when_versions_agree(self) -> None:
        """No drift is reported when the versions agree."""
        info = _distribution_version(source_version="1.2.3", metadata_version="1.2.3")
        assert info.has_drift is False

    @pytest.mark.parametrize(
        ("source", "metadata"),
        [(None, "1.2.3"), ("1.2.3", None), (None, None)],
    )
    def test_has_drift_false_when_a_version_is_unknown(
        self, source: str | None, metadata: str | None
    ) -> None:
        """Drift requires both versions; a missing one is never drift."""
        info = _distribution_version(source_version=source, metadata_version=metadata)
        assert info.has_drift is False


class TestVersionAnnotations:
    """Tests for the rendered editable/drift/mismatch annotations."""

    def _report(
        self,
        *,
        cli: DistributionVersion | None = None,
        sdk: DistributionVersion | None = None,
        requirement: Requirement | None = None,
        satisfied: bool | None = None,
    ) -> VersionReport:
        return VersionReport(
            cli=cli or _distribution_version(name="niki-agent"),
            sdk=sdk or _distribution_version(),
            sdk_requirement=requirement,
            sdk_requirement_satisfied=satisfied,
        )

    def test_cli_annotation_empty_for_normal_install(self) -> None:
        """A non-editable install with agreeing versions gets no annotation."""
        info = _distribution_version(
            name="niki-agent", source_version="0.1.41", metadata_version="0.1.41"
        )
        assert format_cli_version_annotation(info) == ""

    def test_cli_annotation_omits_editable_when_versions_agree(self) -> None:
        """Editable status is shown separately, so an in-sync editable CLI is bare.

        The CLI editable state is rendered by the dedicated `Editable install:`
        line (and doctor's `Install method`), so the inline annotation carries
        only drift — here there is none.
        """
        info = _distribution_version(
            name="niki-agent",
            source_version="0.1.41",
            metadata_version="0.1.41",
            editable=True,
        )
        assert format_cli_version_annotation(info) == ""

    def test_cli_annotation_shows_stale_metadata(self) -> None:
        """An editable CLI with stale metadata shows the drift, not `editable`."""
        info = _distribution_version(
            name="niki-agent",
            source_version="0.1.41",
            metadata_version="0.1.40",
            editable=True,
        )
        assert format_cli_version_annotation(info) == " (installed metadata: 0.1.40)"

    def test_sdk_annotation_empty_for_normal_install(self) -> None:
        """A non-editable SDK whose requirement is satisfied gets no annotation."""
        sdk = _distribution_version(source_version="0.7.0", metadata_version="0.7.0")
        report = self._report(
            sdk=sdk, requirement=Requirement("deepagents==0.7.0"), satisfied=True
        )
        assert format_sdk_version_annotation(report) == ""

    def test_sdk_annotation_shows_stale_metadata_without_mismatch(self) -> None:
        """An editable SDK with stale metadata but a satisfied pin shows drift."""
        sdk = _distribution_version(
            source_version="0.7.1", metadata_version="0.7.0", editable=True
        )
        report = self._report(
            sdk=sdk, requirement=Requirement("deepagents>=0.7.0"), satisfied=True
        )
        assert (
            format_sdk_version_annotation(report)
            == " (editable; installed metadata: 0.7.0)"
        )

    def test_sdk_annotation_uses_newer_exact_pin_for_editable_install(self) -> None:
        """A newer exact pin explains why the effective SDK version is ahead."""
        cli = _distribution_version(
            name="niki-agent", editable=True, source_path="/repo/libs/code"
        )
        sdk = _distribution_version(
            source_version="0.6.12",
            metadata_version="0.6.12",
            editable=True,
            source_path="/repo/libs/deepagents",
        )
        report = self._report(
            cli=cli,
            sdk=sdk,
            requirement=Requirement("deepagents==0.7.0a7"),
            satisfied=True,
        )
        assert report.effective_sdk_version == "0.7.0a7"
        assert report.display_sdk_version == "0.7.0a7+editable"
        assert format_sdk_version_annotation(report) == (
            " (workspace HEAD; source marker: 0.6.12)"
        )

    def test_workspace_head_decorates_display_not_comparison_version(self) -> None:
        """The editable marker never changes the requirement comparison value."""
        cli = _distribution_version(
            name="niki-agent", editable=True, source_path="/repo/libs/code"
        )
        sdk = _distribution_version(
            source_version="0.6.12",
            metadata_version="0.6.12",
            editable=True,
            source_path="/repo/libs/deepagents",
        )
        requirement = Requirement("deepagents==0.7.0a7+build")
        report = self._report(
            cli=cli,
            sdk=sdk,
            requirement=requirement,
            satisfied=True,
        )
        comparison = _sdk_requirement_comparison_version(cli, sdk, requirement)
        assert comparison == "0.7.0a7+build"
        assert _requirement_satisfied(requirement, comparison) is True
        assert _display_sdk_version(cli, sdk, requirement) == ("0.7.0a7+build.editable")
        assert report.display_sdk_version == "0.7.0a7+build.editable"

    @pytest.mark.parametrize("source_version", [None, "not-a-version"])
    def test_workspace_head_requires_valid_source_version(
        self, source_version: str | None
    ) -> None:
        """Stale metadata cannot mask a missing or malformed source marker."""
        cli = _distribution_version(
            name="niki-agent", editable=True, source_path="/repo/libs/code"
        )
        sdk = _distribution_version(
            source_version=source_version,
            metadata_version="0.6.12",
            editable=True,
            source_path="/repo/libs/deepagents",
        )
        report = self._report(
            cli=cli,
            sdk=sdk,
            requirement=Requirement("deepagents==0.7.0a7"),
            satisfied=False,
        )
        assert report.sdk_is_workspace_head is False
        expected = source_version or "0.6.12"
        assert report.effective_sdk_version == expected
        assert report.sdk_source_version_invalid is True
        assert report.sdk_requirement_mismatch is True

    @pytest.mark.parametrize(
        ("pin", "satisfied", "mismatch"),
        [
            pytest.param("0.6.10", False, True, id="older-pin"),
            pytest.param("0.6.12", True, False, id="equal-pin"),
        ],
    )
    def test_workspace_head_requires_pin_newer_than_marker(
        self, pin: str, satisfied: bool, mismatch: bool
    ) -> None:
        """A sibling pin that is not strictly newer never overrides the marker.

        Guards the `pinned > max(markers)` comparison: an older pin must still
        surface as a mismatch, and an equal pin must not decorate the version as
        workspace HEAD.
        """
        cli = _distribution_version(
            name="niki-agent", editable=True, source_path="/repo/libs/code"
        )
        sdk = _distribution_version(
            source_version="0.6.12",
            metadata_version="0.6.12",
            editable=True,
            source_path="/repo/libs/deepagents",
        )
        report = self._report(
            cli=cli,
            sdk=sdk,
            requirement=Requirement(f"deepagents=={pin}"),
            satisfied=satisfied,
        )
        # The observed source marker wins; the pin neither masks it nor adds the
        # `+editable` workspace-HEAD decoration.
        assert report.sdk_is_workspace_head is False
        assert report.effective_sdk_version == "0.6.12"
        assert report.display_sdk_version == "0.6.12"
        assert report.sdk_requirement_mismatch is mismatch

    def test_ranged_requirement_never_overrides_sibling_marker(self) -> None:
        """A ranged sibling requirement stays a mismatch instead of overriding.

        `_exact_pin` returns `None` for a range, so the workspace-HEAD override
        must decline and the marker must still read as a mismatch.
        """
        cli = _distribution_version(
            name="niki-agent", editable=True, source_path="/repo/libs/code"
        )
        sdk = _distribution_version(
            source_version="0.6.12",
            metadata_version="0.6.12",
            editable=True,
            source_path="/repo/libs/deepagents",
        )
        report = self._report(
            cli=cli,
            sdk=sdk,
            requirement=Requirement("deepagents>=0.7,<0.8"),
            satisfied=False,
        )
        assert report.sdk_is_workspace_head is False
        assert report.effective_sdk_version == "0.6.12"
        assert report.display_sdk_version == "0.6.12"
        assert (
            "required by niki-agent: <0.8,>=0.7 — mismatch"
            in format_sdk_version_annotation(report)
        )

    @pytest.mark.parametrize(
        ("cli_editable", "sdk_editable"),
        [
            pytest.param(True, False, id="sdk-not-editable"),
            pytest.param(False, True, id="cli-not-editable"),
        ],
    )
    def test_workspace_sibling_requires_both_editable(
        self, cli_editable: bool, sdk_editable: bool
    ) -> None:
        """The sibling shape only holds when both installs are editable."""
        cli = _distribution_version(
            name="niki-agent",
            editable=cli_editable,
            source_path="/repo/libs/code",
        )
        sdk = _distribution_version(
            editable=sdk_editable,
            source_path="/repo/libs/deepagents",
        )
        assert _editable_sdk_is_cli_workspace_sibling(cli, sdk) is False

    def test_sdk_annotation_shows_ranged_requirement_mismatch(self) -> None:
        """A ranged requirement keeps its full specifier in the mismatch note."""
        sdk = _distribution_version(source_version="0.6.12", metadata_version="0.6.12")
        report = self._report(
            sdk=sdk,
            requirement=Requirement("deepagents>=0.7,<0.8"),
            satisfied=False,
        )
        annotation = format_sdk_version_annotation(report)
        assert "required by niki-agent: <0.8,>=0.7 — mismatch" in annotation

    def test_sdk_annotation_shows_editable_drift_without_mismatch(self) -> None:
        """An editable source matching the exact pin is healthy.

        Stale metadata is still shown for diagnostics.
        """
        sdk = _distribution_version(
            source_version="0.7.0a7", metadata_version="0.6.12", editable=True
        )
        report = self._report(
            sdk=sdk, requirement=Requirement("deepagents==0.7.0a7"), satisfied=True
        )
        assert report.effective_sdk_version == "0.7.0a7"
        assert format_sdk_version_annotation(report) == (
            " (editable; installed metadata: 0.6.12)"
        )


class TestWithEditableLocalVersion:
    """Tests for `_with_editable_local_version`.

    Mirrors the SDK's `test_version.py` cases for
    `deepagents._version._with_editable_local_version`; both helpers must stamp
    identical strings so the two `lc_versions` entries in a trace match.
    """


class TestRequirementSatisfied:
    """Tests for comparing a declared requirement against installed metadata."""

    def test_exact_pin_satisfied(self) -> None:
        assert _requirement_satisfied(Requirement("deepagents==0.7.0"), "0.7.0") is True

    def test_exact_pin_unsatisfied(self) -> None:
        assert (
            _requirement_satisfied(Requirement("deepagents==0.7.0a7"), "0.6.12")
            is False
        )

    def test_ranged_requirement_satisfied(self) -> None:
        assert (
            _requirement_satisfied(Requirement("deepagents>=0.7,<0.8"), "0.7.3") is True
        )

    def test_ranged_requirement_unsatisfied(self) -> None:
        assert (
            _requirement_satisfied(Requirement("deepagents>=0.7,<0.8"), "0.6.12")
            is False
        )

    def test_prerelease_metadata_is_evaluated(self) -> None:
        """A prerelease installed version is compared, not silently dropped."""
        assert (
            _requirement_satisfied(Requirement("deepagents==0.7.0a7"), "0.7.0a7")
            is True
        )

    @pytest.mark.parametrize(
        ("requirement", "metadata"),
        [(None, "0.7.0"), (Requirement("deepagents==0.7.0"), None), (None, None)],
    )
    def test_missing_inputs_are_inconclusive(
        self, requirement: Requirement | None, metadata: str | None
    ) -> None:
        """A missing requirement or version yields `None`, not a boolean."""
        assert _requirement_satisfied(requirement, metadata) is None

    def test_unparseable_version_is_inconclusive(self) -> None:
        """An unparseable installed version degrades to `None` rather than raising."""
        assert (
            _requirement_satisfied(Requirement("deepagents==0.7.0"), "not-a-version")
            is None
        )


class TestSdkRequirementFromCli:
    """Tests for reading the declared `deepagents` requirement from metadata."""

    def _dist(self, requires: list[str]) -> MagicMock:
        dist = MagicMock()
        dist.requires = requires
        return dist

    def test_returns_base_requirement(self) -> None:
        """The unconditional `deepagents` dependency is returned."""
        dist = self._dist(["deepagents==0.7.0a7", "rich>=13"])
        with patch("niki_agent.extras_info.distribution", return_value=dist):
            req = sdk_requirement_from_cli()
        assert req is not None
        assert str(req) == "deepagents==0.7.0a7"

    def test_skips_extras_gated_requirement(self) -> None:
        """An `extra`-gated `deepagents` entry is not treated as the base pin."""
        dist = self._dist(['deepagents[extra]==9.9.9; extra == "foo"'])
        with patch("niki_agent.extras_info.distribution", return_value=dist):
            assert sdk_requirement_from_cli() is None

    def test_skips_unparseable_entries(self) -> None:
        """A malformed `Requires-Dist` entry is skipped, not fatal."""
        dist = self._dist(["=not a requirement=", "deepagents>=0.7,<0.8"])
        with patch("niki_agent.extras_info.distribution", return_value=dist):
            req = sdk_requirement_from_cli()
        assert req is not None
        assert str(req.specifier) == "<0.8,>=0.7"

    def test_missing_distribution_returns_none(self) -> None:
        """A missing distribution yields `None` rather than raising."""
        with patch(
            "niki_agent.extras_info.distribution",
            side_effect=PackageNotFoundError("niki-agent"),
        ):
            assert sdk_requirement_from_cli() is None

    @pytest.mark.parametrize(
        "error",
        [
            UnicodeDecodeError("utf-8", b"\xff", 0, 1, "invalid start byte"),
            TypeError("malformed metadata"),
        ],
    )
    def test_unreadable_requirements_return_none(self, error: Exception) -> None:
        """Unreadable requirement metadata is inconclusive rather than fatal."""
        dist = MagicMock()
        type(dist).requires = PropertyMock(side_effect=error)
        with patch("niki_agent.extras_info.distribution", return_value=dist):
            assert sdk_requirement_from_cli() is None

    def test_returns_applicable_marked_requirement(self) -> None:
        """A base `deepagents` dep whose marker applies is still returned."""
        dist = self._dist(['deepagents==0.7.0a7; python_version >= "3.0"'])
        with patch("niki_agent.extras_info.distribution", return_value=dist):
            req = sdk_requirement_from_cli()
        assert req is not None
        assert str(req.specifier) == "==0.7.0a7"

    def test_unevaluable_marker_is_treated_as_applicable(self) -> None:
        """A marker that fails to evaluate must not silently drop the pin.

        Dropping it would hide the SDK requirement mismatch this check exists to
        surface, so an unexpected evaluation error is treated as applicable.
        """
        dist = self._dist(['deepagents==0.7.0a7; python_version >= "3.0"'])

        def boom(_self: object, *_a: object, **_k: object) -> bool:
            msg = "marker blew up"
            raise ValueError(msg)

        with (
            patch("niki_agent.extras_info.distribution", return_value=dist),
            patch("packaging.markers.Marker.evaluate", boom),
        ):
            req = sdk_requirement_from_cli()
        assert req is not None
        assert str(req.specifier) == "==0.7.0a7"


class TestCollectVersionInfo:
    """Tests for the version collectors that read the live environment."""

    def test_collect_cli_normal_install(self) -> None:
        """A non-editable CLI reports source and metadata with no editable flag."""
        with (
            patch(
                "niki_agent.extras_info._read_cli_source_version",
                return_value="0.1.41",
            ),
            patch("niki_agent.extras_info.pkg_version", return_value="0.1.41"),
            patch(
                "niki_agent.extras_info._cli_editable_info",
                return_value=(False, None),
            ),
        ):
            info = collect_cli_version_info()
        assert info.source_version == "0.1.41"
        assert info.metadata_version == "0.1.41"
        assert info.editable is False
        assert info.status == "resolved"
        assert info.has_drift is False

    def test_collect_cli_editable_stale_metadata(self) -> None:
        """An editable CLI surfaces the stale-metadata drift."""
        with (
            patch(
                "niki_agent.extras_info._read_cli_source_version",
                return_value="0.1.41",
            ),
            patch("niki_agent.extras_info.pkg_version", return_value="0.1.40"),
            patch(
                "niki_agent.extras_info._cli_editable_info",
                return_value=(True, "~/src/deepagents/libs/code"),
            ),
        ):
            info = collect_cli_version_info()
        assert info.primary_version == "0.1.41"
        assert info.metadata_version == "0.1.40"
        assert info.editable is True
        assert info.source_path == "~/src/deepagents/libs/code"
        assert info.has_drift is True

    def test_collect_cli_metadata_missing_resolves_from_source(self) -> None:
        """Absent CLI metadata still resolves via the source version."""
        with (
            patch(
                "niki_agent.extras_info._read_cli_source_version",
                return_value="0.1.41",
            ),
            patch(
                "niki_agent.extras_info.pkg_version",
                side_effect=PackageNotFoundError("niki-agent"),
            ),
            patch(
                "niki_agent.extras_info._cli_editable_info",
                return_value=(False, None),
            ),
        ):
            info = collect_cli_version_info()
        assert info.status == "resolved"
        assert info.metadata_version is None
        assert info.primary_version == "0.1.41"

    def test_collect_cli_metadata_and_source_missing_is_not_installed(self) -> None:
        """No source and no metadata reports `not_installed`."""
        with (
            patch(
                "niki_agent.extras_info._read_cli_source_version",
                return_value=None,
            ),
            patch(
                "niki_agent.extras_info.pkg_version",
                side_effect=PackageNotFoundError("niki-agent"),
            ),
            patch(
                "niki_agent.extras_info._cli_editable_info",
                return_value=(False, None),
            ),
        ):
            info = collect_cli_version_info()
        assert info.status == "not_installed"
        assert info.primary_version is None

    def test_collect_cli_unexpected_error_with_source_resolves(self) -> None:
        """An unexpected metadata failure still resolves when source is present."""
        with (
            patch(
                "niki_agent.extras_info._read_cli_source_version",
                return_value="0.1.41",
            ),
            patch(
                "niki_agent.extras_info.pkg_version",
                side_effect=RuntimeError("boom"),
            ),
            patch(
                "niki_agent.extras_info._cli_editable_info",
                return_value=(False, None),
            ),
        ):
            info = collect_cli_version_info()
        assert info.status == "resolved"
        assert info.primary_version == "0.1.41"

    def test_collect_cli_unexpected_error_without_source_is_error(self) -> None:
        """An unexpected metadata failure with no source reports `error`."""
        with (
            patch(
                "niki_agent.extras_info._read_cli_source_version",
                return_value=None,
            ),
            patch(
                "niki_agent.extras_info.pkg_version",
                side_effect=RuntimeError("boom"),
            ),
            patch(
                "niki_agent.extras_info._cli_editable_info",
                return_value=(False, None),
            ),
        ):
            info = collect_cli_version_info()
        assert info.status == "error"
        assert info.primary_version is None

    def test_collect_sdk_reports_the_vendored_copy(self) -> None:
        """The SDK collector reads the vendored version and never drifts."""
        from niki_agent.sdk._version import __version__ as vendored

        info = collect_sdk_version_info()
        assert info.status == "resolved"
        assert info.metadata_version == vendored
        assert info.source_version == vendored
        assert info.editable is False
        assert info.has_drift is False
        assert info.name == "niki_agent.sdk"

    def test_collect_version_report_flags_requirement_mismatch(self) -> None:
        """The aggregated report flags an unsatisfied declared SDK requirement."""
        code_dist = MagicMock()
        code_dist.requires = ["deepagents==0.7.0a7"]

        def fake_version(name: str) -> str:
            return {"deepagents": "0.6.12", "niki-agent": "0.1.41"}[name]

        with (
            patch("niki_agent.extras_info.pkg_version", side_effect=fake_version),
            patch("niki_agent.extras_info.distributions", return_value=[]),
            patch("niki_agent.extras_info.distribution", return_value=code_dist),
            patch(
                "niki_agent.extras_info._cli_editable_info",
                return_value=(False, None),
            ),
        ):
            report = collect_version_report()
        assert report.sdk_requirement is not None
        assert report.sdk_requirement_mismatch is True

    def test_collect_version_report_uses_no_network_or_subprocess(
        self, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        """Collecting the report must not open sockets or spawn subprocesses."""
        import socket
        import subprocess

        def _forbidden(*_args: object, **_kwargs: object) -> None:
            msg = "version collection must stay offline and process-free"
            raise AssertionError(msg)

        monkeypatch.setattr(socket, "socket", _forbidden)
        monkeypatch.setattr(subprocess, "run", _forbidden)
        monkeypatch.setattr(subprocess, "Popen", _forbidden)

        report = collect_version_report()
        assert isinstance(report, VersionReport)


class TestCompositeExtraMembers:
    """`COMPOSITE_EXTRA_MEMBERS` must mirror `pyproject.toml`."""

    def test_lookup_finds_providing_composite(self) -> None:
        """A member maps back to the composite that provides it."""
        assert composite_extras_providing("ollama") == frozenset({"all-providers"})
        assert composite_extras_providing("daytona") == frozenset({"all-sandboxes"})

    def test_lookup_is_empty_for_unbundled_extras(self) -> None:
        """Extras no composite expands to return an empty set."""
        assert composite_extras_providing("media") == frozenset()
        assert composite_extras_providing("all-providers") == frozenset()

    def test_mapping_covers_every_composite(self) -> None:
        """Every self-referencing extra in pyproject.toml has a member set.

        A composite is exactly an extra that expands through a single
        `niki-agent[...]` requirement, so a new one added to
        `pyproject.toml` must appear here rather than being silently treated
        as an ordinary extra.
        """
        composites = {
            extra
            for extra, requirements in _optional_dependencies().items()
            if len(requirements) == 1
            and canonicalize_name(Requirement(requirements[0]).name)
            == canonicalize_name("niki-agent")
        }
        assert composites == set(COMPOSITE_EXTRA_MEMBERS)

    def test_members_match_pyproject_expansion(self) -> None:
        """The mapped members equal the extras the composite requirement selects.

        Removal reports which composite provides an unremovable member, so a
        composite that gained or lost providers in `pyproject.toml` without a
        matching update here would name the wrong extra — or stay silent and
        report an installed member as "not installed".
        """
        optional = _optional_dependencies()
        for composite, members in COMPOSITE_EXTRA_MEMBERS.items():
            requirements = optional[composite]
            assert len(requirements) == 1, (
                f"{composite} must expand through a single self-referencing "
                f"requirement, got {requirements}"
            )
            declared = Requirement(requirements[0]).extras
            assert declared == set(members), (
                f"pyproject.toml {composite} selects {sorted(declared)}, "
                f"COMPOSITE_EXTRA_MEMBERS has {sorted(members)}"
            )


class TestBaseDependencyExtras:
    """`BASE_DEPENDENCY_EXTRAS` must mirror `pyproject.toml`."""

    def test_matches_extras_fully_covered_by_base_dependencies(self) -> None:
        """The set equals the extras whose packages are all base dependencies.

        Removal refuses these because deselecting them frees nothing. An extra
        that gains or loses that property in `pyproject.toml` without a matching
        update here would either be refused for no reason or accepted and then
        rebuild the environment to no effect.
        """
        data = tomllib.loads(_PYPROJECT_PATH.read_text(encoding="utf-8"))
        base_packages = {
            canonicalize_name(Requirement(requirement).name)
            for requirement in data["project"]["dependencies"]
        }
        covered = {
            extra
            for extra, requirements in _optional_dependencies().items()
            # An empty extra (e.g. `quickjs`, kept for backwards-compatible
            # install commands) adds nothing to remove either, so it belongs
            # here too.
            if all(
                canonicalize_name(Requirement(requirement).name) in base_packages
                for requirement in requirements
            )
        }
        assert covered == set(BASE_DEPENDENCY_EXTRAS)
