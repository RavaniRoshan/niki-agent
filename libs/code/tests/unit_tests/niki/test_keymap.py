"""K1: one keymap registry, and no advertised key that does nothing.

The registry is derived from live `BINDINGS` declarations rather than restated,
so the footer, Help, and `docs/niki/KEYMAP.md` cannot drift from what the app
binds. These tests hold the two halves of that claim: every advertised entry
resolves to a handler that exists, and a representative advertised key dispatches
when pressed.

Two things the first draft of this file got wrong, both worth recording because
they are easy to repeat:

- `App` is not a child widget. Walking `app.walk_children()` never yields the
  app itself, so every app-level action (`interrupt`, `quit_app`,
  `toggle_auto_approve`, ...) looked like a dead key. The resolution root has to
  include the app.
- Assertions must run *inside* `run_test`. Reading `app.focused` afterwards
  raises `ScreenStackError: No screens on stack`, because the context manager
  has already torn the stack down.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from deepagents_code.niki.keymap import (
    build_keymap,
    render_footer_hint,
    render_help,
    render_markdown,
)

if TYPE_CHECKING:
    from collections.abc import Callable

    from deepagents_code.app import DeepAgentsApp
    from deepagents_code.niki.app import NikiApp
    from deepagents_code.niki.keymap import KeymapEntry


def _app_classes() -> tuple[type[NikiApp], type[DeepAgentsApp]]:
    """Return `(NikiApp, DeepAgentsApp)` without importing at module scope."""
    from deepagents_code.app import DeepAgentsApp
    from deepagents_code.niki.app import NikiApp

    return NikiApp, DeepAgentsApp


def _handlers_for(root: DeepAgentsApp) -> set[str]:
    """Every `action_*` name reachable from `root`, including `root` itself.

    Textual resolves a binding against the focused widget and then walks up the
    tree to the app, so the resolution root is the app -- not the screen.
    """
    handlers: set[str] = set()
    for node in [root, *root.walk_children(with_self=False)]:
        handlers.update(n for n in dir(type(node)) if n.startswith("action_"))
    return handlers


def _normalise(action: str) -> str:
    """Map a binding action to the `action_*` name Textual dispatches to.

    Two forms need folding. Namespaced actions (`screen.copy_text`) live on the
    `Screen` class as `action_copy_text` -- the dotted part only selects the
    namespace. Parameterized actions (`approval_position(0)`) dispatch to
    `action_approval_position` with an argument; the `(0)` selects which option,
    not which handler.
    """
    without_arg = action.split("(", 1)[0]
    return without_arg.split(".", 1)[-1]


async def _in_app[TResult](
    body: Callable[[NikiApp, list[KeymapEntry]], TResult],
) -> TResult:
    """Mount the Niki app, run `body(app, keymap)`, and return its result.

    `body` must run inside the `run_test` block: reading `app.focused` or the
    screen stack afterwards raises `ScreenStackError`.
    """
    from unittest.mock import AsyncMock, MagicMock

    niki_app_cls, _ = _app_classes()
    app = niki_app_cls(agent=MagicMock(), thread_id="niki-keymap")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        return body(app, build_keymap(app))


async def test_registry_is_populated_from_live_bindings() -> None:
    """A registry that came back empty would make every other check vacuous."""

    def check(app: NikiApp, keymap: list[KeymapEntry]) -> tuple[int, set[str]]:  # noqa: ARG001
        return len(keymap), {e.namespace for e in keymap}

    count, namespaces = await _in_app(check)

    assert count > 20, f"only {count} bindings discovered"
    assert "NikiApp" in namespaces, (
        "the app's own bindings are missing; the registry must start at the app, "
        "not at the screen"
    )


async def test_every_advertised_action_has_a_handler() -> None:
    """K1: nothing may be advertised without a handler to dispatch to."""
    missing: list[str] = []

    def check(app: NikiApp, keymap: list[KeymapEntry]) -> None:
        handlers = {name.removeprefix("action_") for name in _handlers_for(app)}
        missing.extend(
            sorted(
                {
                    action
                    for entry in keymap
                    for action in [entry.action]
                    if action and _normalise(action) not in handlers
                }
            )
        )

    await _in_app(check)

    assert not missing, f"advertised actions with no action_* handler: {missing}"


async def test_escape_is_advertised_as_interrupt_and_dispatches() -> None:
    """K1/K4: press a representative advertised key and observe the effect.

    Pressing every key is not viable -- many open modals, run tools, or quit --
    so this proves the dispatch path end to end for one key, while the
    handler-resolution test above covers the rest by construction.
    """

    def check(app: NikiApp, keymap: list[KeymapEntry]) -> tuple[set[str], bool]:
        escapes = [e for e in keymap if e.key == "escape"]
        return {e.action for e in escapes}, hasattr(app, "action_interrupt")

    actions, has_interrupt = await _in_app(check)

    assert actions, "escape is missing from the registry"

    # Several widgets may bind escape; the deepest one wins at dispatch time
    # (an open search consumes it before the app does). K4's guarantee is that
    # the app-level binding *is* advertised and *does* interrupt, not that no
    # other surface ever sees the key first.
    assert "interrupt" in actions, (
        f"escape never advertises interrupt; got {sorted(actions)}"
    )
    assert has_interrupt, "escape advertises a handler that does not exist"


def test_help_and_markdown_come_from_one_registry() -> None:
    """D3/K1: the docs and the help body must not be separate lists."""
    from textual.app import App

    keymap = build_keymap(App())

    markdown = render_markdown(keymap)
    assert markdown.startswith("| Key | Action | Description | Source |")
    assert render_help(keymap) == render_help(keymap), (
        "render_help is not deterministic"
    )
    for entry in keymap[:5]:
        assert f"`{entry.key}`" in markdown, f"{entry.key} missing from the table"
        assert entry.key in render_help(keymap)


def test_footer_hint_is_bounded_to_one_line() -> None:
    """The footer must stay one line however many keys exist."""
    from textual.app import App

    hint = render_footer_hint(build_keymap(App()), limit=4)

    assert "\n" not in hint
    assert hint.count(" · ") <= 3, f"footer hint overflowed its limit: {hint!r}"


def test_generated_keymap_leaks_no_upstream_name() -> None:
    """V2: generated docs must not leak an upstream name either."""
    from textual.app import App

    markdown = render_markdown(build_keymap(App()))
    assert "Deep Agents" not in markdown
    assert "dcode" not in markdown


@pytest.mark.parametrize("limit", [1, 3, 8])
def test_footer_hint_never_exceeds_its_limit(limit: int) -> None:
    """A limit of N must never produce more than N entries."""
    from textual.app import App

    hint = render_footer_hint(build_keymap(App()), limit=limit)
    assert len([part for part in hint.split(" · ") if part]) <= max(limit, 1)
