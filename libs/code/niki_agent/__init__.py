"""Deep Agents Code - Interactive AI coding assistant."""

from __future__ import annotations

import logging
import sys
from typing import TYPE_CHECKING

from niki_agent._debug import configure_debug_logging
from niki_agent._debug_buffer import install_log_buffer
from niki_agent._home_error import NikiHomeError
from niki_agent._version import __version__

if TYPE_CHECKING:
    from collections.abc import Callable

install_log_buffer(logging.getLogger(__name__))  # noqa: RUF067  # attach the always-on tail first so warnings from configure_debug_logging are captured
configure_debug_logging(logging.getLogger(__name__))  # noqa: RUF067  # package logger must be configured before child modules emit logs; sets the final level over the buffer's INFO floor

__all__ = [
    "__version__",
    "cli_main",  # noqa: F822  # resolved lazily by __getattr__
]


def __getattr__(name: str) -> Callable[[], None]:
    """Lazy import for `cli_main` to avoid loading `main.py` at package import.

    `main.py` pulls in `argparse`, signal handling, and other startup machinery
    that isn't needed when submodules like `config` or `widgets` are
    imported directly.

    Returns:
        The requested callable.

    Raises:
        AttributeError: If *name* is not a lazily-provided attribute.

    Note:
        Any import error other than an unresolvable profile location
        propagates unchanged; `NikiHomeError` is reported as a message
        plus exit 2 instead of a traceback.
    """
    if name == "cli_main":
        try:
            from niki_agent.main import cli_main
        except NikiHomeError as exc:
            # `_paths` resolves the profile at import, so a bad NIKI_HOME
            # or an unresolvable home surfaces here. Report it as a message
            # rather than a traceback: the user has a value to fix, and every
            # module that imports `_paths` would fail the same way.
            message = str(exc)

            def cli_main() -> None:
                print(f"niki: {message}", file=sys.stderr)  # noqa: T201
                raise SystemExit(2)

        return cli_main
    msg = f"module {__name__!r} has no attribute {name!r}"
    raise AttributeError(msg)
