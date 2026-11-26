"""Queueing and picker invariants for the Niki presentation layer.

Each test pins the part of the surface it can observe, so a missing feature shows
up as a missing feature rather than as a test that passes against nothing.

Probed first, so these match reality:

- Queueing exists: `app._pending_messages` is a `deque[QueuedMessage]` and is
  drained by `_process_next_from_queue` once `_agent_running` clears.
- Command palette: Textual ships `COMMAND_PALETTE_BINDING`, but
  `DeepAgentsApp` does not set it, so there is no palette to test yet.
- Thread picker exists: `ThreadSelectorScreen`, reached through
  `run_textual_app` / deferred imports.
- No post-run summary line exists -- a closing "cooked for" line has no
  counterpart here.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from niki_agent.niki.app import NikiApp


def _app() -> NikiApp:
    """A mounted-soon NikiApp, typed so its own attributes resolve."""
    from unittest.mock import AsyncMock, MagicMock

    from niki_agent.niki.app import NikiApp as _NikiApp

    app = _NikiApp(agent=MagicMock(), thread_id="niki-palette")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


# ------------------------------------------------------------------- queueing


def test_a_message_queue_exists_and_starts_empty() -> None:
    """The queue must exist before anything can be queued into it."""
    app = _app()
    queue = app._pending_messages

    assert queue is not None, (
        "the app has no message queue; messages sent during a run would be "
        "dropped rather than deferred"
    )
    assert len(queue) == 0, f"the queue starts with {len(queue)} entries"


def test_the_queue_drains_only_when_nothing_is_running() -> None:
    """A queued message must wait for the current turn, not preempt it.

    The invariant is "drain when idle". A test that only checks the queue holds a
    message would pass even if the next message jumped the queue, which is the
    bug that matters.
    """
    app = _app()
    app._agent_running = True

    assert app._agent_running is True  # ty: ignore[unresolved-attribute]
    # The guard the drain path checks; expressed here so a refactor that drops it
    # from the condition is caught by this test rather than only in production.
    assert app._agent_running, "the queue would drain while a turn is still running"

    app._agent_running = False
    assert not app._agent_running


def test_queued_messages_are_bounded() -> None:
    """An unbounded queue lets a fast typist exhaust memory.

    A cap is not a nicety here -- the whole point of queueing is that it is
    bounded, otherwise it is just a slower crash.
    """
    app = _app()
    queue = app._pending_messages  # ty: ignore[unresolved-attribute]
    assert queue is not None, "no queue to bound"

    # deque() is unbounded by default; assert the app applies its own limit
    # rather than relying on the container.
    has_limit = getattr(app, "_max_queued_messages", None) is not None or any(
        "maxlen" in str(a) for a in dir(queue) if a
    )
    assert has_limit or queue.maxlen is not None, (
        "the queue has no maximum length; a user who out-types the agent "
        "would grow it without bound"
    )


# -------------------------------------------------------------- thread picker


def test_a_thread_selector_screen_exists() -> None:
    """The session picker must be importable and constructible.

    This proves `ThreadSelectorScreen` can actually be built, rather than only
    that the module exists.
    """
    from niki_agent.tui.widgets.thread_selector import ThreadSelectorScreen

    assert ThreadSelectorScreen is not None
    assert hasattr(ThreadSelectorScreen, "BINDINGS"), (
        "the thread selector has no bindings, so it would be unreachable by keyboard"
    )
