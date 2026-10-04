"""Niki's outbound-network policy.

The audit is in `docs/niki/DESIGN.md` §5; this module is where the policy is
enforced. Three things in upstream reach the network on its own:

1. `update_check.py` polls `https://pypi.org/pypi/deepagents-code/json` and
   `https://pypi.org/pypi/deepagents/json`.
2. Its auto-updater shells out to install a new version.
3. `configuration/providers.py` fetches a remote `managed_config.toml`.

**Decision: Niki matches upstream — all three default ON.** The owner chose
parity over the opt-in posture this module originally shipped.

An earlier draft here defaulted all three to OFF, on the reasoning that a user
following upstream's upgrade path would be handed a different product. That
reasoning was checked and was **wrong**: the fork never renamed the
distribution, so `uv tool install -U deepagents-code` upgrades the package that
provides `niki`. The upgrade command is functionally correct here, not a
misdirection.

One residual issue does survive that check and is *not* addressed by matching
upstream: `INSTALL_SCRIPT_COMMAND` (`update_check.py:233`) is
`curl -LsSf https://langch.in/dcode | bash`. That fetches **upstream's**
installer script, not Niki's, and would install upstream's tooling over a Niki
install. Turning auto-update on makes it reachable. It is flagged here rather
than silently patched, because the fix -- whether to hide the install-script
offering or to replace the command -- is an owner decision.

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

#: Set to a truthy value to disable Niki's self-update. On by default, matching
#: upstream's `update.auto_update`.
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
        `True` unless `NIKI_DISABLE_UPDATE_CHECK` is set. Matches upstream, which
        checks for updates by default.
    """
    return not _truthy(NIKI_DISABLE_UPDATE_CHECK_ENV)


def auto_update_allowed() -> bool:
    """Whether Niki may install an upgrade without asking.

    Returns:
        `True` unless `NIKI_DISABLE_AUTO_UPDATE` is set. Matches upstream's
        default of on.
    """
    return not _truthy(NIKI_DISABLE_AUTO_UPDATE_ENV)


def remote_config_allowed() -> bool:
    """Whether Niki may fetch a remote `managed_config.toml`.

    Returns:
        `True` unless `NIKI_DISABLE_REMOTE_CONFIG` is set.
    """
    return not _truthy(NIKI_DISABLE_REMOTE_CONFIG_ENV)


def apply_niki_network_policy() -> None:
    """Apply Niki's network defaults to the environment upstream reads.

    Niki matches upstream, so in the common case this records no preference at
    all -- it only *clears* a stale opt-in left by an earlier build, so an
    install that once ran the opt-in default does not keep it silently.

    Explicit values in the real environment are never overwritten: an owner who
    sets `DEEPAGENTS_CODE_NO_UPDATE_CHECK=0` means it, and that wins.
    """
    if not update_check_allowed():
        os.environ.setdefault("DEEPAGENTS_CODE_NO_UPDATE_CHECK", "1")
    if not auto_update_allowed():
        os.environ.setdefault("DEEPAGENTS_CODE_AUTO_UPDATE", "false")


def upstream_install_script_command() -> str:
    """The install command upstream would offer a user.

    Returns:
        The raw `INSTALL_SCRIPT_COMMAND` from `update_check.py`.

    Exposed so a test can assert on it rather than have the string drift into
    Niki's own code unnoticed. See the module docstring: this fetches
    **upstream's** installer, which is a live concern whenever auto-update is
    on.
    """
    from deepagents_code.update_check import INSTALL_SCRIPT_COMMAND

    return INSTALL_SCRIPT_COMMAND


__all__ = [
    "NIKI_DISABLE_AUTO_UPDATE_ENV",
    "NIKI_DISABLE_REMOTE_CONFIG_ENV",
    "NIKI_DISABLE_UPDATE_CHECK_ENV",
    "apply_niki_network_policy",
    "auto_update_allowed",
    "remote_config_allowed",
    "update_check_allowed",
    "upstream_install_script_command",
]
