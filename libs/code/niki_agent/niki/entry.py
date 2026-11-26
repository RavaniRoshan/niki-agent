"""Niki's presentation entry point.

`niki` is Niki's only console script. This module adds what `cli_main` alone
does not: Niki's key-reference help screen and Niki's glyph vocabulary. The
module entry point (`python -m niki_agent`) goes straight to `cli_main`, so it
keeps the argparse help and default glyphs.

The entry point deliberately does the cheap things itself and delegates the rest:

- `--version` / `-v` is answered here, so it costs one `importlib.metadata`
  lookup rather than importing the app.
- `--help` / `-h` renders the generated keymap screen without starting Textual.
- The network policy is applied before the app reads its settings.
- Everything else hands off to `cli_main`, so tools, permissions, approvals,
  slash commands, and session storage are shared with the module entry point.

`NikiApp` is the app class, with Niki's title and presentation defaults. Wiring
it in is done by `install_niki_app_class`, which must run before the app is
constructed.
"""

from __future__ import annotations

import sys

from niki_agent.niki.branding import version_line
from niki_agent.niki.network import apply_niki_network_policy

_VERSION_FLAGS = frozenset({"-v", "--version"})
_HELP_FLAGS = frozenset({"-h", "--help"})


def install_niki_app_class() -> None:
    """Point the app factory at `NikiApp`.

    `niki_agent.app.run_textual_app` constructs `DeepAgentsApp(...)`
    directly, so a subclass has to be substituted before that module-level name
    is looked up. Rebinding the attribute is contained to this process and is
    undone by nothing, because a process runs one app.
    """
    import niki_agent.app as app_module
    from niki_agent.niki.app import NikiApp

    # Ignored deliberately. Upstream annotates this name as the exact
    # `DeepAgentsApp` type; substituting a subclass that adds only presentation
    # defaults is the whole point of this function, so the assignment cannot be
    # made to typecheck without lying about it. Narrow and local by
    # construction -- process-scoped, and applied before the app is built.
    app_module.DeepAgentsApp = NikiApp  # ty: ignore[invalid-assignment]


def install_niki_glyphs() -> None:
    """Point upstream's glyph lookup at Niki's motion language.

    Every spinner in the app reads `get_glyphs()` (`loading.py:31`,
    `messages.py:2207`), so binding it once gives Niki a single coherent glyph
    vocabulary across the whole TUI without editing any widget.

    Process-scoped and installed only from `niki_main`, so the deprecated
    aliases keep the previous glyphs byte-for-byte.
    """
    from dataclasses import replace

    import niki_agent.config as config_module
    from niki_agent.config import get_glyphs
    from niki_agent.niki.motion import spinner_frames_for_this_terminal

    # Substitute upstream's FULL Glyphs with only `spinner_frames` replaced.
    # A narrow stand-in satisfies the type but crashes every widget that reads
    # another glyph field; see the note in `niki/motion.py`.
    resolved = replace(get_glyphs(), spinner_frames=spinner_frames_for_this_terminal())
    config_module._glyphs_cache = resolved  # ty: ignore[invalid-assignment]


def niki_main() -> None:
    """Run Niki Agent. Registered as the `niki` console script."""
    if len(sys.argv) == 2 and sys.argv[1] in _VERSION_FLAGS:  # noqa: PLR2004
        # Answered before any heavy import, matching upstream's own fast path.
        print(version_line())  # noqa: T201 - console script output, not a library
        return

    if len(sys.argv) == 2 and sys.argv[1] in _HELP_FLAGS:  # noqa: PLR2004
        # The authoritative flag list, then Niki's generated key reference.
        from niki_agent.niki.help import print_help
        from niki_agent.ui import console, show_help

        show_help()
        print_help(console)
        return

    apply_niki_network_policy()
    install_niki_glyphs()
    install_niki_app_class()

    from niki_agent.main import cli_main
    from niki_agent.niki.runtime_mode import niki_runtime_scope

    with niki_runtime_scope():
        cli_main()


__all__ = ["install_niki_app_class", "niki_main"]
