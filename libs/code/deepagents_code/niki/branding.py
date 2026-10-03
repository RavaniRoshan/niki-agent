"""Niki's user-visible identity strings.

Kept in one module so a rename is one edit, and so the leak test has a single
place to check against. The rules the fork follows:

- User-visible chrome says **Niki Agent**. Never "Deep Agents" or "dcode".
- `Deep Agents` is still named where honesty requires it: the `--version` line
  ("built on Deep Agents"), the About view, and license notices. Pretending the
  SDK is something else would be a lie about what the software is.
- `dcode` stays a working command. `niki` is added beside it, not instead of it.
"""

from __future__ import annotations

from importlib.metadata import PackageNotFoundError, version as _dist_version

PRODUCT_NAME = "Niki Agent"
"""What the header, welcome screen, and help text call this product."""

VERSION_LINE_SUFFIX = ", built on Deep Agents"
"""Appended to `--version`. Attribution, not branding -- see the module docstring."""

SDK_CREDIT = "Niki Agent is built on Deep Agents (MIT)."


def product_version() -> str:
    """Return the installed `deepagents-code` version, or `unknown` if absent.

    Read through `importlib.metadata` rather than importing the package: the
    repo's own startup rule is that `-v` must not pay for heavy imports, and
    `import deepagents_code` is measured at 18.7 ms before anything else.

    Returns:
        The installed distribution version, or `"unknown"` when the package is
        not installed.
    """
    try:
        return _dist_version("deepagents-code")
    except PackageNotFoundError:
        return "unknown"


def version_line() -> str:
    """Build the full `--version` string.

    Returns:
        For example `Niki Agent 0.1.80, built on Deep Agents`.
    """
    return f"{PRODUCT_NAME} {product_version()}{VERSION_LINE_SUFFIX}"


__all__ = [
    "PRODUCT_NAME",
    "SDK_CREDIT",
    "VERSION_LINE_SUFFIX",
    "product_version",
    "version_line",
]
