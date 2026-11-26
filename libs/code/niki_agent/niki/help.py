"""Niki's help screen, generated from the keymap registry.

`ui.show_help()` hand-maintains the full CLI flag screen, and `libs/code/AGENTS.md`
requires it to stay in sync with `main.parse_args()`. That screen stays as the
authoritative flag list. This screen is the *key* reference: generated from
`build_keymap`, so a key can only appear here if a mounted widget class actually
binds it.

The `niki` console script answers `--help` with this screen; `dcode` and
`deepagents-code` keep the argparse screen while they remain as aliases.

This is also what closes K1: the footer, Help, and the generated keymap doc all
read the same registry.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from textual.app import App

from niki_agent.niki.branding import SDK_CREDIT, product_version
from niki_agent.niki.keymap import build_keymap

if TYPE_CHECKING:
    from collections.abc import Sequence

    from rich.console import Console

    from niki_agent.niki.keymap import KeymapEntry

#: The commands worth listing above the key table. Upstream owns the full set;
#: these are the ones a first-run Niki user reaches for.
HEADLINE_COMMANDS: tuple[tuple[str, str], ...] = (
    ("niki", "Start an interactive thread"),
    ("niki agents <list|reset>", "Manage agents"),
    ("niki skills <list|info>", "Manage agent skills"),
    ("niki --version", "Show the version"),
    ("niki --help", "Show this screen"),
)

_SECTION_HEADINGS = ("Usage:", "Keys:", "Slash commands:")


def _section(rows: Sequence[tuple[str, str]]) -> list[str]:
    """Pad a two-column block so the descriptions line up.

    Args:
        rows: Label/description pairs.

    Returns:
        One indented, padded line per row; empty when there is nothing to show.
    """
    if not rows:
        return []
    width = max(len(left) for left, _ in rows)
    return [f"  {left:<{width}}  {right}" for left, right in rows]


def _compose(entries: list[KeymapEntry]) -> str:
    """Assemble the screen body from a binding registry.

    Args:
        entries: The live bindings to document.

    Returns:
        The help body as plain text, with no colour markup.
    """
    lines: list[str] = [
        "",
        f"Niki Agent v{product_version()}",
        SDK_CREDIT,
        "",
        "Usage:",
        *_section(HEADLINE_COMMANDS),
        "",
        "Keys:",
        *_section([(e.key, e.description or e.action) for e in entries]),
        "",
        "Slash commands: press / in the composer.",
        "",
    ]
    return "\n".join(lines)


def render_help() -> str:
    """Build the help screen from a bare Textual app's bindings.

    Returns:
        The help body as plain text.
    """
    return _compose(build_keymap(App()))


def render_help_for(app: App) -> str:
    """Build the help screen from a live app's bindings.

    Args:
        app: A mounted app, so the screen lists what this session can do.

    Returns:
        The help body as plain text.
    """
    return _compose(build_keymap(app))


def print_help(console: Console) -> None:
    """Print the help screen to `console`, with a little colour.

    Args:
        console: The Rich console to print to.
    """
    console.print("[bold]Niki Agent[/bold]")
    for line in render_help().splitlines()[2:]:
        if line.strip() in _SECTION_HEADINGS:
            console.print(f"[bold]{line.strip()}[/bold]")
        else:
            console.print(line)


__all__ = [
    "HEADLINE_COMMANDS",
    "print_help",
    "render_help",
    "render_help_for",
]
