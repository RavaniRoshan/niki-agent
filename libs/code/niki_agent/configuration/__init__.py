"""Managed and user configuration: paths, providers, and merge.

Writes live in `niki_agent.configuration.writer` and are imported from
there directly, so no writer symbol is re-exported here.
"""

from niki_agent.configuration.paths import (
    ResolvedManagedPath,
    managed_config_path,
    resolve_managed_path,
)
from niki_agent.configuration.service import (
    ManagedConfigError,
    ManagedHealth,
    ManagedPolicyError,
    get_config_sources,
    invalidate_config_sources,
    managed_config_status,
    managed_declaration,
    managed_health,
    managed_policy_violations,
    require_healthy_managed_config,
)
from niki_agent.configuration.types import ProviderHealth, ProviderStatus

__all__ = [
    "ManagedConfigError",
    "ManagedHealth",
    "ManagedPolicyError",
    "ProviderHealth",
    "ProviderStatus",
    "ResolvedManagedPath",
    "get_config_sources",
    "invalidate_config_sources",
    "managed_config_path",
    "managed_config_status",
    "managed_declaration",
    "managed_health",
    "managed_policy_violations",
    "require_healthy_managed_config",
    "resolve_managed_path",
]
