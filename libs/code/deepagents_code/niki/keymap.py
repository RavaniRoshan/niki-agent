"""The single keymap registry (K1).

Checklist row K1 asks for one registry that the footer, Help, and the slash
popup all read, so none of them can advertise a dead action. Upstream declares
`BINDINGS` per class and merges them at runtime, but `ui.show_help()` is
hand-maintained text -- which is precisely how a product ends up advertising a
key that does nothing.

This module derives the keymap from the **live** binding declarations instead of
restating them. Two consequences worth stating:

- A key appears in the footer and in `docs/niki/KEYMAP.md` only because some
  class actually binds it. There is no second list to drift.
- `check_action` is honoured. Textual hides a binding whose action is
  unavailable, so a row marked `show=False` upstream stays out of the hints
  instead of being advertised and doing nothing.

`build_keymap` reads `BINDINGS` off the classes, which is Textual's public
class-level declaration API. It does not reach into the merged runtime binding
tree, because that is private and its shape is not part of Textual's contract.
"""

from __future__ import annotations

from collections.abc import Iterable
from dataclasses import dataclass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Iterator, Sequence

    from textual.binding import Binding
    from textual.dom import DOMNode


@dataclass(frozen=True)
class KeymapEntry:
    """One advertised key.

    Attributes:
        key: The key sequence, e.g. `"ctrl+t"`.
        action: The action name, e.g. `"toggle_subagent_panel"`.
        description: What the key does, for the footer and Help.
        show: Whether upstream marks this binding as footer-visible.
        namespace: Where the binding was declared, e.g. `"DeepAgentsApp"`.
    """

    key: str
    action: str
    description: str
    show: bool
    namespace: str


def _bindings_of(node: DOMNode) -> Sequence[Binding]:
    """Return the `BINDINGS` declared directly on `node`'s class."""
    declared = getattr(type(node), "BINDINGS", ())
    return tuple(declared) if isinstance(declared, Iterable) else ()


def collect_bindings(root: DOMNode) -> Iterator[tuple[str, Binding]]:
    """Yield `(namespace, binding)` for `root` and every mounted descendant.

    Walks the mounted tree rather than the class hierarchy: only bindings that
    exist in the current screen are advertised, so a key that belongs to a modal
    the user has not opened does not appear in the transcript footer.
    """
    for node in root.walk_children(with_self=True):
        for binding in _bindings_of(node):
            yield type(node).__name__, binding


def build_keymap(root: DOMNode, *, visible_only: bool = False) -> list[KeymapEntry]:
    """Build the registry for `root`'s currently mounted widget tree.

    Args:
        root: Usually the app or its screen.
        visible_only: Drop bindings upstream marks `show=False`. Off by default,
            because **every** upstream binding sets `show=False` -- upstream
            hand-renders its own footer hints instead of using Textual's. With
            the filter on, the registry comes back empty and Help would document
            nothing. The `show` flag is recorded per entry so a surface can still
            select on it.

    Returns:
        Entries sorted by key, with duplicates resolved in favour of the most
        specific (deepest) declaration, matching what Textual would dispatch.
    """
    entries: dict[tuple[str, str], KeymapEntry] = {}
    for namespace, binding in collect_bindings(root):
        if visible_only and not binding.show:
            continue
        key = binding.key
        if isinstance(key, tuple):
            key = "+".join(key)
        entry = KeymapEntry(
            key=str(key),
            action=binding.action or "",
            description=binding.description or "",
            show=bool(binding.show),
            namespace=namespace,
        )
        # A later, deeper declaration wins: that is the one Textual dispatches.
        entries[entry.key, entry.action] = entry
    return sorted(entries.values(), key=lambda e: (e.key, e.action))


def render_footer_hint(keymap: Iterable[KeymapEntry], *, limit: int = 6) -> str:
    """Render a single-line footer hint from the registry.

    Args:
        keymap: Entries to advertise.
        limit: Maximum entries before the rest are dropped.

    Returns:
        A single line such as `"ctrl+t panels · ? help"`, or `""` when empty.
    """
    parts = [f"{e.key} {e.description}".strip() for e in keymap if e.key]
    return " · ".join(parts[:limit])


def render_help(keymap: Iterable[KeymapEntry]) -> str:
    """Render the help body from the registry.

    Args:
        keymap: Entries to document.

    Returns:
        One `key    description` line per entry, sorted by key.
    """
    rows = sorted(keymap, key=lambda e: (e.key, e.action))
    if not rows:
        return "No key bindings are currently active."
    width = max(len(e.key) for e in rows)
    return "\n".join(f"{e.key:<{width}}  {e.description}".rstrip() for e in rows)


def render_markdown(keymap: Iterable[KeymapEntry]) -> str:
    """Render the registry as the `docs/niki/KEYMAP.md` table.

    Args:
        keymap: Entries to document.

    Returns:
        A Markdown table with a Key, Action, Description, and Source column.
    """
    header = ["| Key | Action | Description | Source |", "| --- | --- | --- | --- |"]
    body = [
        f"| `{e.key}` | `{e.action}` | {e.description} | `{e.namespace}` |"
        for e in sorted(keymap, key=lambda entry: (entry.key, entry.action))
    ]
    return "\n".join(header + body)


__all__ = [
    "KeymapEntry",
    "build_keymap",
    "collect_bindings",
    "render_footer_hint",
    "render_help",
    "render_markdown",
]
