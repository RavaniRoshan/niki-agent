"""The `niki` console script.

A new entry point beside `dcode`, not a replacement. `dcode` and
`deepagents-code` keep working exactly as before; this only adds a third name so
the product can say its own name in chrome, `--version`, and help text without
rewriting upstream's strings.

The entry point deliberately does the cheap things itself and delegates the rest:

- `--version` / `-v` is answered here, so it costs one `importlib.metadata`
  lookup rather than importing the app. Upstream's own `-v` fast path is
  349 ms cold; this path does not import `deepagents_code` at all.
- The network policy is applied before upstream reads its settings.
- Everything else hands off to upstream's `cli_main`, so tools, permissions,
  approvals, slash commands, and session storage are unchanged by construction.

`NikiApp` is what upstream would have built, with Niki's title and the
presentation defaults. Wiring it in is done by `install_niki_app_class`, which
must run before upstream constructs the app.
"""

from __future__ import annotations

import sys

from deepagents_code.niki.branding import version_line
from deepagents_code.niki.network import apply_niki_network_policy

_VERSION_FLAGS = frozenset({"-v", "--version"})
_HELP_FLAGS = frozenset({"-h", "--help"})


def install_niki_app_class() -> None:
    """Point upstream's app factory at `NikiApp`.

    `deepagents_code.app.run_textual_app` constructs `DeepAgentsApp(...)`
    directly, so a subclass has to be substituted before that module-level name
    is looked up. Rebinding the attribute is contained to this process and is
    undone by nothing, because a process runs one app.
    """
    import deepagents_code.app as app_module
    from deepagents_code.niki.app import NikiApp

    # Ignored deliberately. Upstream annotates this name as the exact
    # `DeepAgentsApp` type; substituting a subclass that adds only presentation
    # defaults is the whole point of this function, so the assignment cannot be
    # made to typecheck without lying about it. Narrow and local by
    # construction -- process-scoped, and applied before the app is built.
    app_module.DeepAgentsApp = NikiApp  # ty: ignore[invalid-assignment]


def niki_main() -> None:
    """Run Niki Agent. Registered as the `niki` console script."""
    if len(sys.argv) == 2 and sys.argv[1] in _VERSION_FLAGS:  # noqa: PLR2004
        # Answered before any heavy import, matching upstream's own fast path.
        print(version_line())  # noqa: T201 - console script output, not a library
        return

    if len(sys.argv) == 2 and sys.argv[1] in _HELP_FLAGS:  # noqa: PLR2004
        # Niki's own screen, generated from the keymap registry. Upstream's
        # hand-maintained help is left byte-for-byte intact for `dcode`.
        from deepagents_code.niki.help import print_help
        from deepagents_code.ui import console

        print_help(console)
        return

    apply_niki_network_policy()
    install_niki_app_class()

    from deepagents_code.main import cli_main

    cli_main()


__all__ = ["install_niki_app_class", "niki_main"]
