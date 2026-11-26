"""Niki's user-visible identity strings.

Kept in one module so a rename is one edit, and so the brand gate has a single
place to check against. The rules:

- User-visible chrome says **Niki Agent**. Never an upstream or pre-rename
  product name.
- Attribution lives in `NOTICE`, `LICENSE`, the vendored SDK's
  `PROVENANCE.md`, and the About view -- not in the version line.
- `niki` is the only console script; there are no compatibility aliases.
"""

from __future__ import annotations

from importlib.metadata import PackageNotFoundError, version as _dist_version

PRODUCT_NAME = "Niki Agent"
"""What the header, welcome screen, and help text call this product."""

VERSION_LINE_SUFFIX = ""
"""Appended to `--version`. Empty: attribution belongs in NOTICE, not chrome."""

SDK_CREDIT = "Niki Agent includes the Deep Agents SDK (MIT)."


def product_version() -> str:
    """Return the installed `niki-agent` version, or `unknown` if absent.

    Read through `importlib.metadata` rather than importing the package: the
    repo's own startup rule is that `-v` must not pay for heavy imports.

    Returns:
        The installed distribution version, or `"unknown"` when the package is
        not installed.
    """
    try:
        return _dist_version("niki-agent")
    except PackageNotFoundError:
        return "unknown"


def version_line() -> str:
    """Build the full `--version` string.

    Returns:
        For example `Niki Agent 1.0.0`.
    """
    return f"{PRODUCT_NAME} {product_version()}{VERSION_LINE_SUFFIX}"


__all__ = [
    "PRODUCT_NAME",
    "SDK_CREDIT",
    "VERSION_LINE_SUFFIX",
    "product_version",
    "version_line",
]
