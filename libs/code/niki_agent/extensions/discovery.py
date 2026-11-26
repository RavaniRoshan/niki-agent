"""Resolve authorized Python extension sources into entry files."""

from __future__ import annotations

import importlib.metadata
import importlib.util
import logging
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING

from niki_agent._env_vars import EXPERIMENTAL, is_env_truthy
from niki_agent.extensions.registry import SourceInfo, SourceScope

if TYPE_CHECKING:
    from collections.abc import Iterable, Sequence

    from niki_agent.plugins.models import PluginInstance

logger = logging.getLogger(__name__)

ENTRY_POINT_GROUP = "niki.extensions"

LEGACY_ENTRY_POINT_GROUP = "dcode.extensions"
"""Entry-point group declared before the product rename.

Installed distributions that have not re-declared their extensions yet are
still discovered through this group; each one is named in a single
`INFO` deprecation line so its author can move to `ENTRY_POINT_GROUP`.
"""


@dataclass(frozen=True, slots=True)
class DiscoveryResult:
    """Authorized sources and non-fatal resolution errors."""

    sources: tuple[SourceInfo, ...] = ()
    errors: tuple[str, ...] = ()


def user_extensions_dir() -> Path:
    """Return the user-wide loose-file extension directory."""
    from niki_agent.model_config import DEFAULT_CONFIG_PATH

    return DEFAULT_CONFIG_PATH.parent / "extensions"


def project_extensions_dir(project_root: Path) -> Path:
    """Return the extension directory beneath `project_root`.

    Prefers `.niki/extensions`, reading the legacy `.deepagents/extensions` only
    while the project has no `.niki` directory.
    """
    from niki_agent._paths import resolve_project_dir

    return resolve_project_dir(project_root) / "extensions"


def _canonical(path: Path) -> Path:
    return path.expanduser().resolve()


def _source(path: Path, scope: SourceScope, *, package: bool = False) -> SourceInfo:
    return SourceInfo(_canonical(path), is_package=package, scope=scope)


def _scan(directory: Path, scope: SourceScope) -> tuple[list[SourceInfo], list[str]]:
    try:
        entries = sorted(directory.expanduser().iterdir())
    except FileNotFoundError:
        return [], []
    except OSError as exc:
        return [], [f"Could not scan extension directory {directory}: {exc}"]

    sources: list[SourceInfo] = []
    errors: list[str] = []
    for entry in entries:
        try:
            source = _entry_source(entry, scope)
        except (OSError, RuntimeError) as exc:
            errors.append(f"Could not inspect extension entry {entry}: {exc}")
            continue
        if source is not None:
            sources.append(source)
    return sources, errors


def _entry_source(entry: Path, scope: SourceScope) -> SourceInfo | None:
    if entry.is_file() and entry.suffix == ".py":
        return _source(entry, scope)
    for filename in ("__init__.py", "extension.py"):
        candidate = entry / filename
        if candidate.is_file():
            return _source(candidate, scope, package=True)
    return None


def _resolve_explicit(path: Path, scope: SourceScope) -> DiscoveryResult:
    expanded = path.expanduser()
    try:
        if expanded.is_dir():
            sources, errors = _scan(expanded, scope)
            return DiscoveryResult(tuple(sources), tuple(errors))
        if expanded.is_file() and expanded.suffix == ".py":
            return DiscoveryResult((_source(expanded, scope),))
    except (OSError, RuntimeError):
        return DiscoveryResult(errors=("Could not inspect an extension path",))
    msg = f"Extension path must be an existing Python file or directory: {path}"
    return DiscoveryResult(errors=(msg,))


def _resolve_paths(paths: Iterable[Path], scope: SourceScope) -> DiscoveryResult:
    sources: list[SourceInfo] = []
    errors: list[str] = []
    for path in paths:
        result = _resolve_explicit(path, scope)
        sources.extend(result.sources)
        errors.extend(result.errors)
    return DiscoveryResult(tuple(sources), tuple(errors))


def _plugin_sources(plugins: Sequence[PluginInstance]) -> list[SourceInfo]:
    return [
        SourceInfo(
            _canonical(path),
            is_package=path.name == "__init__.py",
            source_id=plugin.plugin_id,
            version=plugin.version,
            installed_root=_canonical(plugin.root),
        )
        for plugin in plugins
        if plugin.manifest is not None
        for path in plugin.manifest.python_extensions
    ]


def _entry_point_source(entry: importlib.metadata.EntryPoint) -> SourceInfo:
    module = entry.value.partition(":")[0]
    spec = importlib.util.find_spec(module)
    if spec is None or spec.origin is None:
        msg = f"Entry point {entry.name!r} does not resolve to a Python module"
        raise ValueError(msg)
    version = entry.dist.version if entry.dist is not None else None
    return SourceInfo(
        _canonical(Path(spec.origin)),
        is_package=spec.submodule_search_locations is not None,
        source_id=f"{entry.name}@entry-point",
        version=version,
        installed_root=_canonical(Path(spec.origin).parent),
    )


def _entry_point_sources() -> DiscoveryResult:
    sources: list[SourceInfo] = []
    errors: list[str] = []
    try:
        current = list(importlib.metadata.entry_points(group=ENTRY_POINT_GROUP))
        legacy = list(importlib.metadata.entry_points(group=LEGACY_ENTRY_POINT_GROUP))
    except (ImportError, OSError, ValueError) as exc:
        msg = f"Could not enumerate extension entry points: {exc}"
        return DiscoveryResult(errors=(msg,))

    ordered = sorted(
        [(entry, False) for entry in current] + [(entry, True) for entry in legacy],
        key=lambda item: (item[0].name, item[0].value, item[1]),
    )
    seen: set[tuple[str, str]] = set()
    reported: set[str] = set()
    for entry, is_legacy in ordered:
        identity = (entry.name, entry.value)
        if identity in seen:
            continue
        seen.add(identity)
        if is_legacy:
            _log_legacy_entry_point(entry, reported)
        try:
            sources.append(_entry_point_source(entry))
        except (ImportError, OSError, ValueError) as exc:
            errors.append(
                f"Could not resolve extension entry point {entry.name}: {exc}"
            )
    return DiscoveryResult(tuple(sources), tuple(errors))


def _log_legacy_entry_point(
    entry: importlib.metadata.EntryPoint, reported: set[str]
) -> None:
    """Log the legacy-group deprecation once per distribution.

    Args:
        entry: An entry point found under `LEGACY_ENTRY_POINT_GROUP`.
        reported: Distributions already named in this scan; mutated in place.
    """
    distribution = entry.dist.name if entry.dist is not None else entry.value
    if distribution in reported:
        return
    reported.add(distribution)
    logger.info(
        "Extension entry point group %r is deprecated; distribution %r should "
        "declare its extensions in %r",
        LEGACY_ENTRY_POINT_GROUP,
        distribution,
        ENTRY_POINT_GROUP,
    )


def _deduplicate(sources: Iterable[SourceInfo]) -> tuple[SourceInfo, ...]:
    unique: dict[Path, SourceInfo] = {}
    for source in sources:
        unique.setdefault(source.path, source)
    return tuple(unique.values())


def discover_extensions(
    *,
    plugins: Sequence[PluginInstance] = (),
    config_paths: Sequence[Path] = (),
    cli_paths: Sequence[Path] = (),
    project_dir: Path | None = None,
) -> DiscoveryResult:
    """Resolve all authorized sources in deterministic load order.

    Returns:
        Canonically deduplicated sources and isolated errors.
    """
    if not is_env_truthy(EXPERIMENTAL):
        return DiscoveryResult()
    user_sources, user_errors = _scan(user_extensions_dir(), SourceScope.USER)
    config = _resolve_paths(config_paths, SourceScope.USER)
    cli = _resolve_paths(cli_paths, SourceScope.TEMPORARY)
    entries = _entry_point_sources()
    project = (
        _resolve_explicit(project_dir, SourceScope.PROJECT)
        if project_dir is not None
        else DiscoveryResult()
    )
    sources = _deduplicate(
        (
            *user_sources,
            *config.sources,
            *cli.sources,
            *_plugin_sources(plugins),
            *entries.sources,
            *project.sources,
        )
    )
    errors = (
        *user_errors,
        *config.errors,
        *cli.errors,
        *entries.errors,
        *project.errors,
    )
    return DiscoveryResult(sources, errors)
