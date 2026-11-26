"""Version information and lightweight constants for `niki-agent`."""

# Keep the `x-release-please-version` annotation — release-please uses it to
# bump `__version__` in sync with `pyproject.toml` on every release PR.
__version__ = "1.1.0"  # x-release-please-version

DOCS_URL = "https://github.com/RavaniRoshan/niki-agent"
"""URL for `niki-agent` documentation."""

PYPI_URL = "https://pypi.org/pypi/niki-agent/json"
"""PyPI JSON API endpoint for version checks."""

SDK_PYPI_URL = PYPI_URL
"""Release-metadata endpoint for the vendored SDK.

The SDK is vendored, so it has no separate distribution. The product's own
PyPI metadata is consulted; a version that predates the product simply
resolves to no release age, which the caller already handles.
"""

CHANGELOG_URL = (
    "https://github.com/RavaniRoshan/niki-agent/blob/niki-agent/libs/code/CHANGELOG.md"
)
"""URL for the full changelog."""

USER_AGENT = f"niki-agent/{__version__} update-check"
"""User-Agent header sent with PyPI requests."""
