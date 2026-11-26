"""Niki's outbound-network policy.

The audit is in `docs/niki/DESIGN.md` §5; this module is where the policy is
enforced. Three things reach the network on their own:

1. `update_check.py` polls `https://pypi.org/pypi/niki-agent/json`.
2. Its auto-updater shells out to install a new version.
3. `configuration/providers.py` fetches a remote `managed_config.toml`.

**Decision: all three default ON**, matching the behaviour Niki's users expect
from a terminal coding agent. The owner chose parity over the opt-in posture
this module originally shipped.

Niki is now its own distribution (`niki-agent`), so the upgrade command and the
install script both point at Niki. `INSTALL_SCRIPT_COMMAND` is
`https://raw.githubusercontent.com/RavaniRoshan/niki-agent/niki-agent/libs/code/scripts/install.sh`,
which installs `niki-agent` rather than any upstream tooling.

The opt-*out* variables below exist so an owner or a managed deployment can turn
any of this off without editing code. They use `setdefault`, so a value already
present in the environment always wins over anything this module does.

Auth flows are deliberately untouched. Nothing here intercepts, reads, or
changes how a provider key is obtained.
"""

from __future__ import annotations

import os

#: Set to a truthy value to disable Niki's update *checking* (read-only).
NIKI_DISABLE_UPDATE_CHECK_ENV = "NIKI_DISABLE_UPDATE_CHECK"

#: Set to a truthy value to disable Niki's self-update.
NIKI_DISABLE_AUTO_UPDATE_ENV = "NIKI_DISABLE_AUTO_UPDATE"

#: Set to a truthy value to disable the remote managed-config fetch.
NIKI_DISABLE_REMOTE_CONFIG_ENV = "NIKI_DISABLE_REMOTE_CONFIG"

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
        `True` unless `NIKI_DISABLE_UPDATE_CHECK` is set.
    """
    return not _truthy(NIKI_DISABLE_UPDATE_CHECK_ENV)


def auto_update_allowed() -> bool:
    """Whether Niki may install an upgrade without asking.

    Returns:
        `True` unless `NIKI_DISABLE_AUTO_UPDATE` is set.
    """
    return not _truthy(NIKI_DISABLE_AUTO_UPDATE_ENV)


def remote_config_allowed() -> bool:
    """Whether Niki may fetch a remote `managed_config.toml`.

    Returns:
        `True` unless `NIKI_DISABLE_REMOTE_CONFIG` is set.
    """
    return not _truthy(NIKI_DISABLE_REMOTE_CONFIG_ENV)


def apply_niki_network_policy() -> None:
    """Apply Niki's network defaults to the environment the app reads.

    Explicit values in the real environment are never overwritten: an owner who
    sets `NIKI_NO_UPDATE_CHECK=0` means it, and that wins.
    """
    if not update_check_allowed():
        os.environ.setdefault("NIKI_NO_UPDATE_CHECK", "1")
    if not auto_update_allowed():
        os.environ.setdefault("NIKI_AUTO_UPDATE", "false")


def install_script_command() -> str:
    """The install command Niki offers a user.

    Returns:
        The raw `INSTALL_SCRIPT_COMMAND` from `update_check.py`.

    Exposed so a test can assert on the promoted command rather than have the
    string drift into Niki's own code unnoticed.
    """
    from niki_agent.update_check import INSTALL_SCRIPT_COMMAND

    return INSTALL_SCRIPT_COMMAND


__all__ = [
    "NIKI_DISABLE_AUTO_UPDATE_ENV",
    "NIKI_DISABLE_REMOTE_CONFIG_ENV",
    "NIKI_DISABLE_UPDATE_CHECK_ENV",
    "apply_niki_network_policy",
    "auto_update_allowed",
    "install_script_command",
    "remote_config_allowed",
    "update_check_allowed",
]
