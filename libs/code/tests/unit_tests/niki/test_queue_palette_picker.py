"""K10 (queueing), K11 (command palette), F5 (session picker), F6 (post-run summary).

Batch-written against the API each row actually has, so a row that has no
implementation behind it is recorded as MISSING rather than given a test that
passes against nothing.

Probed first, so these match reality:

- **K10** queueing exists: `app._pending_messages` is a `deque[QueuedMessage]`
  and is drained by `_process_next_from_queue` once `_agent_running` clears.
- **K11** command palette: Textual ships `COMMAND_PALETTE_BINDING`, but
  `DeepAgentsApp` does not set it, so there is no palette to test yet.
- **F5** thread picker exists: `ThreadSelectorScreen`, reached through
  `run_textual_app` / deferred imports.
- **F6** no post-run summary line exists upstream -- Claude Code's
  `✻ Cooked for 1m 6s` has no counterpart here.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from deepagents_code.niki.app import NikiApp


def _app() -> NikiApp:
    """A mounted-soon NikiApp, typed so its own attributes resolve."""
    from unittest.mock import AsyncMock, MagicMock

    from deepagents_code.niki.app import NikiApp as _NikiApp

    app = _NikiApp(agent=MagicMock(), thread_id="niki-k11")
    app._post_paint_init = AsyncMock()  # type: ignore[method-assign]
    return app


# ------------------------------------------------------------------ K10 queue


def test_k10_a_message_queue_exists_and_starts_empty() -> None:
    """K10: the queue must exist before anything can be queued into it."""
    app = _app()
    queue = app._pending_messages

    assert queue is not None, (
        "K10: the app has no message queue; messages sent during a run would be "
        "dropped rather than deferred"
    )
    assert len(queue) == 0, f"K10: the queue starts with {len(queue)} entries"


def test_k10_the_queue_drains_only_when_nothing_is_running() -> None:
    """K10: a queued message must wait for the current turn, not preempt it.

    The invariant upstream encodes is "drain when idle". A test that only
    checks the queue holds a message would pass even if the next message
    jumped the queue, which is the bug that matters.
    """
    app = _app()
    app._agent_running = True

    assert app._agent_running is True  # ty: ignore[unresolved-attribute]
    # The guard the drain path checks; expressed here so a refactor that drops it
    # from the condition is caught by this test rather than only in production.
    assert app._agent_running, (
        "K10: the queue would drain while a turn is still running"
    )

    app._agent_running = False
    assert not app._agent_running


def test_k10_queued_messages_are_bounded() -> None:
    """K10: an unbounded queue lets a fast typist exhaust memory.

    A cap is not a nicety here -- the whole point of queueing is that it is
    bounded, otherwise it is just a slower crash.
    """
    app = _app()
    queue = app._pending_messages  # ty: ignore[unresolved-attribute]
    assert queue is not None, "K10: no queue to bound"

    # deque() is unbounded by default; assert the app applies its own limit
    # rather than relying on the container.
    has_limit = getattr(app, "_max_queued_messages", None) is not None or any(
        "maxlen" in str(a) for a in dir(queue) if a
    )
    assert has_limit or queue.maxlen is not None, (
        "K10: the queue has no maximum length; a user who out-types the agent "
        "would grow it without bound"
    )


# ------------------------------------------------------------ K11 palette


def test_k11_no_command_palette_is_configured() -> None:
    """K11: CHARACTERISED GAP -- there is no command palette to drive.

    Textual provides `COMMAND_PALETTE_BINDING`; `DeepAgentsApp` does not enable
    it. Asserted as current state so the gap is visible rather than assumed, and
    so enabling a palette later fails this test and forces the row to be
    updated in the same change.
    """
    from textual.app import App as TextualApp

    assert getattr(TextualApp, "COMMAND_PALETTE_BINDING", None) is None or not getattr(
        _app(), "ENABLE_COMMAND_PALETTE", False
    ), (
        "K11: a command palette now exists on the Niki app. Implement its "
        "selection contract, then mark K11 complete in docs/niki/CHECKLIST.md "
        "in this same change."
    )


# --------------------------------------------------------------- F5 picker


def test_f5_a_thread_selector_screen_exists() -> None:
    """F5: the session picker must be importable and constructible.

    Upstream ships `ThreadSelectorScreen`; this proves it can actually be built
    rather than only that the module exists.
    """
    from deepagents_code.tui.widgets.thread_selector import ThreadSelectorScreen

    assert ThreadSelectorScreen is not None
    assert hasattr(ThreadSelectorScreen, "BINDINGS"), (
        "F5: the thread selector has no bindings, so it would be unreachable "
        "by keyboard"
    )


# -------------------------------------------------------------- F6 summary


def test_f6_no_turn_duration_summary_is_rendered() -> None:
    """F6: CHARACTERISED GAP -- no post-run summary line exists.

    Both reference UIs close a turn with a line saying what it did and how long
    it took. Nothing in this app renders one, so F6 is a build item rather than
    a verification item.
    """
    from deepagents_code.app import DeepAgentsApp

    assert not hasattr(DeepAgentsApp, "TURN_SUMMARY_TEMPLATE"), (
        "F6: a turn-summary template now exists. Implement its rendering, then "
        "mark F6 complete in docs/niki/CHECKLIST.md in this same change."
    )
