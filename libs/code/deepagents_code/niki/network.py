"""Niki's outbound-network policy.

The audit is in `docs/niki/DESIGN.md` §5; this module is where the policy is
enforced. Three things in upstream reach the network on its own:

1. `update_check.py` polls `https://pypi.org/pypi/deepagents-code/json` and
   `https://pypi.org/pypi/deepagents/json`.
2. Its auto-updater shells out to install a new version.
3. `configuration/providers.py` fetches a remote `managed_config.toml`.

For Niki all three default to **off**. The reason is not squeamishness about
telemetry -- it is that upstream's upgrade path is `uv tool install -U
deepagents-code` and `curl -LsSf https://langch.in/dcode | bash`. A Niki user who
followed either would be silently handed a different product. Shipping a fork
whose UI tells users to upgrade the thing it forked *from* is a correctness bug,
not a branding one.

Each setting stays available and documented so an owner can turn it on
knowingly. Upstream's own env vars are honored, but Niki's off-by-default
resolution runs first.

Auth flows are deliberately untouched. Nothing here intercepts, reads, or
changes how a provider key is obtained.
"""

from __future__ import annotations

import os

#: Set to a truthy value to re-enable Niki's update *checking* (read-only).
NIKI_ALLOW_UPDATE_CHECK_ENV = "NIKI_ALLOW_UPDATE_CHECK"

#: Set to a truthy value to re-enable Niki's self-update. Off even upstream-on.
NIKI_ALLOW_AUTO_UPDATE_ENV = "NIKI_ALLOW_AUTO_UPDATE"

#: Set to a truthy value to re-enable the remote managed-config fetch.
NIKI_ALLOW_REMOTE_CONFIG_ENV = "NIKI_ALLOW_REMOTE_CONFIG"

_TRUTHY = frozenset({"1", "true", "yes", "on"})


def _truthy(name: str) -> bool:
    """Read a boolean environment variable.

    Returns:
        `True` when the variable is set to 1, true, yes, or on.
    """
    return os.environ.get(name, "").strip().lower() in _TRUTHY


def update_check_allowed() -> bool:
    """Whether Niki may contact PyPI to look for a newer version.

    Returns:
        `False` unless the owner has explicitly opted in via
        `NIKI_ALLOW_UPDATE_CHECK`.
    """
    return _truthy(NIKI_ALLOW_UPDATE_CHECK_ENV)


def auto_update_allowed() -> bool:
    """Whether Niki may install an upgrade without asking.

    Returns:
        `False` unless `NIKI_ALLOW_AUTO_UPDATE` is set. This is *stricter* than
        upstream, whose `update.auto_update` defaults to on.
    """
    return _truthy(NIKI_ALLOW_AUTO_UPDATE_ENV)


def remote_config_allowed() -> bool:
    """Whether Niki may fetch a remote `managed_config.toml`.

    Returns:
        `False` unless `NIKI_ALLOW_REMOTE_CONFIG` is set.
    """
    return _truthy(NIKI_ALLOW_REMOTE_CONFIG_ENV)


def apply_niki_network_policy() -> None:
    """Force Niki's network defaults into the environment upstream reads.

    Upstream resolves its settings through `_resolve_update_setting`, which ranks
    managed config above environment above `config.toml`. Setting the environment
    here makes Niki's value win for everything that is *not* managed-policy, and
    the managed layer is still free to say otherwise -- so a site that genuinely
    manages its own fleet is not overridden by this fork.

    Only variables that are not already set are written, so an explicit owner
    choice made in the real environment always wins over this default.
    """
    if not update_check_allowed():
        os.environ.setdefault("DEEPAGENTS_CODE_NO_UPDATE_CHECK", "1")
    if not auto_update_allowed():
        # Upstream reads this as "auto update is enabled", so the opt-out is the
        # inverse: recording that the owner did not ask for it.
        os.environ.setdefault("DEEPAGENTS_CODE_AUTO_UPDATE", "false")


__all__ = [
    "NIKI_ALLOW_AUTO_UPDATE_ENV",
    "NIKI_ALLOW_REMOTE_CONFIG_ENV",
    "NIKI_ALLOW_UPDATE_CHECK_ENV",
    "apply_niki_network_policy",
    "auto_update_allowed",
    "remote_config_allowed",
    "update_check_allowed",
]
