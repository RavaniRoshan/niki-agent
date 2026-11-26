"""Context marker for Niki-only runtime composition."""

from __future__ import annotations

from contextlib import contextmanager
from contextvars import ContextVar
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Iterator

_NIKI_RUNTIME_ENABLED: ContextVar[bool] = ContextVar(
    "niki_runtime_enabled", default=False
)


@contextmanager
def niki_runtime_scope() -> Iterator[None]:
    """Enable Niki runtime adapters for this call and its async work."""
    token = _NIKI_RUNTIME_ENABLED.set(True)
    try:
        yield
    finally:
        _NIKI_RUNTIME_ENABLED.reset(token)


def is_niki_runtime_enabled() -> bool:
    """Return whether the current invocation was launched through `niki`."""
    return _NIKI_RUNTIME_ENABLED.get()
