"""K4, K10, and L8 through a real streamed turn.

`Esc` and the message queue are turn-control paths: they only do something
while a turn is genuinely in flight. These tests hold a turn open with a
`Gate` and then drive the real ladder, rather than calling the action methods
on an idle app where every branch is a no-op.
"""

from __future__ import annotations

import asyncio
from typing import TYPE_CHECKING

from unit_tests.niki.turn_harness import (
    Gate,
    Raise,
    ScriptedAgent,
    Text,
    arm_turn,
    make_app,
    screen_text,
    wait_idle,
    wait_running,
)

if TYPE_CHECKING:
    from textual.pilot import Pilot

    from deepagents_code.niki.app import NikiApp

SIZE = (100, 32)


async def _wait_until(
    pilot: Pilot[None],
    predicate,  # lambda over app state
    *,
    limit: float = 5.0,
) -> None:
    loop = asyncio.get_running_loop()
    deadline = loop.time() + limit
    while loop.time() < deadline:
        await pilot.pause()
        if predicate():
            await pilot.pause()
            return
        await asyncio.sleep(0.02)
    msg = "timed out waiting for the expected state"
    raise AssertionError(msg)


def _composer_text(app: NikiApp) -> str:
    return str(getattr(app.query_one("#chat-input"), "text", ""))


async def test_k4_escape_cancels_a_running_turn_and_restores_the_prompt() -> None:
    """K4: `Esc` during a turn with no visible output cancels and restores it."""
    gate = asyncio.Event()
    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        agent = ScriptedAgent([Gate(gate), Text("never streamed")])
        await arm_turn(app, pilot, agent)
        await app._submit_input("draft prompt", "normal")
        await wait_running(app, pilot)
        assert not app._active_turn_visible_output_started, (
            "K4: the turn already produced output, so restore would be skipped"
        )

        await pilot.press("escape")
        await _wait_until(pilot, lambda: not app._agent_running)

        restored = _composer_text(app)
        running = bool(app._agent_running)

    assert not running, "K4: Esc did not cancel the running turn"
    assert restored.strip() == "draft prompt", (
        f"K4: the interrupted prompt was not restored to the composer: {restored!r}"
    )


async def test_k4_escape_pops_a_queued_message_before_interrupting() -> None:
    """K4: with a message queued, `Esc` retracts it instead of cancelling."""
    gate = asyncio.Event()
    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        agent = ScriptedAgent([Gate(gate), Text("done")])
        await arm_turn(app, pilot, agent)
        await app._submit_input("first", "normal")
        await wait_running(app, pilot)
        await app._submit_input("second", "normal")
        await pilot.pause()
        assert len(app._pending_messages) == 1, (
            "K4: the second prompt was not queued while the first ran"
        )

        await pilot.press("escape")
        await pilot.pause()

        queued_after = len(app._pending_messages)
        still_running = bool(app._agent_running)
        restored = _composer_text(app)

        gate.set()
        await wait_idle(app, pilot)

    assert queued_after == 0, "K4: Esc did not pop the queued message"
    assert still_running, "K4: popping a queued message also cancelled the turn"
    assert restored.strip() == "second", (
        f"K4: the popped message was not returned to the composer: {restored!r}"
    )


async def test_k4_double_escape_clears_a_composer_draft() -> None:
    """K4: nothing to interrupt, so a second Esc clears the draft."""
    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        composer = app.query_one("#chat-input")
        composer.insert("unsent draft")
        await pilot.pause()

        await pilot.press("escape")
        await pilot.pause()
        armed = bool(app._clear_input_pending)
        after_first = _composer_text(app)

        await pilot.press("escape")
        await pilot.pause()
        cleared = _composer_text(app)

    assert armed, "K4: the first Esc did not arm the draft clear"
    assert after_first == "unsent draft", "K4: the first Esc cleared the draft early"
    assert cleared == "", f"K4: the second Esc did not clear the draft: {cleared!r}"


async def test_k10_a_message_queued_during_a_run_is_shown_and_then_runs() -> None:
    """K10: a prompt submitted mid-turn is surfaced, queued, and drained."""
    from textual.widgets import Static

    gate = asyncio.Event()
    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        agent = ScriptedAgent([Gate(gate), Text("first done")], [Text("second done")])
        await arm_turn(app, pilot, agent)
        await app._submit_input("first", "normal")
        await wait_running(app, pilot)

        await app._submit_input("second", "normal")
        await pilot.pause()
        during = screen_text(app)
        queued = len(app._pending_messages)
        indicator = str(
            app.query_one("#connection-indicator", Static).render()
        )
        queued_widgets = len(app._queued_widgets)

        gate.set()
        await wait_idle(app, pilot)
        after = screen_text(app)
        drained = len(app._pending_messages)
        calls = [repr(call) for call in agent.calls]

    assert queued == 1, f"K10: the second prompt was not queued: {queued}"
    assert queued_widgets == 1, (
        f"K10: the queued prompt got no transcript card: {queued_widgets}"
    )
    assert "1 message queued" in indicator, (
        f"K10: the status indicator does not name the queue: {indicator!r}"
    )
    assert "second" in during, (
        f"K10: the queued prompt is not visible while it waits:\n{during}"
    )
    assert drained == 0, "K10: the queue did not drain after the turn"
    assert "second" in after, (
        f"K10: the queued prompt never reached the transcript:\n{after}"
    )
    assert any("second" in call for call in calls), (
        f"K10: the queued prompt never ran as a turn: {calls}"
    )


async def test_l5_a_turn_crash_logs_a_traceback_and_shows_a_friendly_error(
    monkeypatch, tmp_path
) -> None:
    """L5: a failing turn writes a traceback to the debug log and stays usable."""
    import logging

    from deepagents_code import _debug

    monkeypatch.setenv("DEEPAGENTS_CODE_DEBUG", "1")
    monkeypatch.setenv("DEEPAGENTS_CODE_DEBUG_DIRECTORY", str(tmp_path))
    # Mirror the `.env` reload path: the env var is set after import, so the
    # package logger must be re-configured before the session binds its file.
    package_logger = logging.getLogger("deepagents_code")
    original_level = package_logger.level
    _debug.configure_debug_logging(package_logger)
    try:
        app = make_app()
        async with app.run_test(size=SIZE) as pilot:
            await pilot.pause()
            agent = ScriptedAgent(
                [Text("partial answer"), Raise(RuntimeError("provider exploded"))]
            )
            await arm_turn(app, pilot, agent)
            log_path = _debug.installed_debug_log_path()
            assert log_path is not None, (
                "L5: the debug handler never attached a file for this thread"
            )

            await app._submit_input("go", "normal")
            await _wait_until(pilot, lambda: not app._agent_running)
            rendered = screen_text(app)
            still_running = bool(app._agent_running)

        log_text = log_path.read_text(encoding="utf-8")
    finally:
        _debug._remove_debug_handlers(package_logger, except_path=None)
        package_logger.setLevel(original_level)

    assert not still_running, "L5: the app kept the failed turn marked as running"
    assert "Agent error" in rendered, (
        f"L5: the failure produced no friendly error card:\n{rendered}"
    )
    assert "Traceback" in log_text, "L5: the debug log has no traceback"
    assert "provider exploded" in log_text, (
        "L5: the debug log does not name the original error"
    )


def test_l8_clean_exit_prints_the_resume_summary(monkeypatch) -> None:
    """L8: teardown prints the thread's resume command."""
    from rich.console import Console

    from deepagents_code import main as main_module, sessions

    async def _exists(_thread_id: str) -> bool:
        await asyncio.sleep(0)
        return True

    monkeypatch.setattr(sessions, "thread_exists", _exists)
    monkeypatch.setattr(main_module, "_resume_term_program", lambda: None)
    monkeypatch.setattr(
        "deepagents_code._invocation.invoked_name", lambda: "dcode"
    )
    monkeypatch.setattr(
        "deepagents_code.config.build_langsmith_thread_url", lambda _tid: None
    )

    console = Console(record=True, width=100)
    main_module._render_teardown_thread_hints(
        console, "thread-abc", return_code=0
    )
    output = console.export_text()

    assert "Resume this thread with:" in output, output
    assert "-r thread-abc" in output, output
    assert "non-zero status" not in output, output


def test_l8_failed_exit_adds_the_resume_caveat(monkeypatch) -> None:
    """L8: a non-zero exit warns that resuming may fail."""
    from rich.console import Console

    from deepagents_code import main as main_module, sessions

    async def _exists(_thread_id: str) -> bool:
        await asyncio.sleep(0)
        return True

    monkeypatch.setattr(sessions, "thread_exists", _exists)
    monkeypatch.setattr(main_module, "_resume_term_program", lambda: None)
    monkeypatch.setattr(
        "deepagents_code._invocation.invoked_name", lambda: "dcode"
    )
    monkeypatch.setattr(
        "deepagents_code.config.build_langsmith_thread_url", lambda _tid: None
    )

    console = Console(record=True, width=100)
    main_module._render_teardown_thread_hints(
        console, "thread-abc", return_code=1
    )
    output = console.export_text()

    assert "-r thread-abc" in output, output
    assert "non-zero status" in output, output
