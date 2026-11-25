"""V6/V7/V8/V10/V11 and F2 through a real streamed turn.

Every assertion here runs the app's own turn pipeline against the transport
double in `turn_harness.py`: the status bar, tool rows, diff mount, activity
line, and markdown renderer are the production ones, not re-rendered fixtures.
That is what these rows were missing — a real stream to react to.
"""

from __future__ import annotations

import asyncio
import re
from typing import TYPE_CHECKING

import pytest

from unit_tests.niki.turn_harness import (
    Gate,
    Mutate,
    ScriptedAgent,
    Sleep,
    Text,
    ToolResult,
    ToolUse,
    arm_turn,
    make_app,
    screen_text,
    scroll_end,
    wait_idle,
    wait_running,
)

if TYPE_CHECKING:
    from pathlib import Path

    from textual.app import App
    from textual.pilot import Pilot

SIZE = (100, 32)
DIFF_PATH = "demo.py"


def _loading_text(app: App) -> str:  # type: ignore[type-arg]
    """Return the activity line's visible text, or an empty string."""
    from deepagents_code.tui.widgets.loading import LoadingWidget

    widgets = list(app.query(LoadingWidget))
    if not widgets:
        return ""
    return " ".join(
        str(child.render()) for child in widgets[0].query("Static") if child.display
    )


async def _turn(app: App, pilot: Pilot[None], script: list[object]) -> ScriptedAgent:  # type: ignore[type-arg]
    """Arm the app with a one-turn script and run the prompt to completion."""
    agent = ScriptedAgent(script)
    await arm_turn(app, pilot, agent)
    await app._submit_input("go", "normal")
    await wait_idle(app, pilot)
    await scroll_end(app, pilot)
    return agent


async def test_v6_the_footer_reports_the_model_and_real_usage(monkeypatch) -> None:
    """V6: model and token figures appear only once a real turn supplies them."""
    from deepagents_code import config

    monkeypatch.setattr(config.runtime_state, "model_name", "niki-test-model")
    monkeypatch.setattr(config.runtime_state, "model_provider", "openai")

    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        idle = screen_text(app)
        assert "niki-test-model" in idle, (
            f"V6: the configured model is not in the footer:\n{idle[-400:]}"
        )
        assert "tokens" not in idle.lower(), (
            "V6: the footer claims tokens before any usage was recorded"
        )

        agent = ScriptedAgent(
            [
                Text(
                    "Working on it.",
                    usage={
                        "input_tokens": 1200,
                        "output_tokens": 340,
                        "total_tokens": 1540,
                    },
                )
            ]
        )
        await arm_turn(app, pilot, agent)
        await app._submit_input("go", "normal")
        await wait_idle(app, pilot)
        footer = screen_text(app).splitlines()[-1]
        metrics = " ".join(
            segment.plain
            for segment in app.query_one("#tokens-display").segments
            if segment.plain
        )
        shown_tokens = app._status_bar.tokens
        context_tokens = app._context_tokens

    assert shown_tokens > 0, "V6: a recorded usage chunk left the token count at 0"
    assert context_tokens == shown_tokens, (
        "V6: the footer and the app's cached context count disagree "
        f"({shown_tokens} vs {context_tokens})"
    )
    assert "Tokens:" in metrics, f"V6: the metrics line shows no count: {metrics!r}"
    assert re.search(r"\d", metrics), f"V6: the metrics line has no number: {metrics!r}"
    assert "niki-test-model" in footer, (
        f"V6: the model vanished from the footer after a turn: {footer!r}"
    )


async def test_v7_a_failed_tool_card_stays_visible_and_names_the_error() -> None:
    """V7: a failure must not be folded away into a group summary."""
    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        await _turn(
            app,
            pilot,
            [
                ToolUse("call-1", "bash", {"command": "cat secret"}),
                ToolResult(
                    "call-1",
                    "bash",
                    "permission denied: /root/secret",
                    status="error",
                ),
            ],
        )
        rendered = screen_text(app)

    assert "bash" in rendered, f"V7: the failed card never names the tool:\n{rendered}"
    assert "permission denied" in rendered, (
        f"V7: the failure text is not on screen without expanding anything:\n{rendered}"
    )


async def test_v7_a_long_error_expands_without_a_click() -> None:
    """V7: a long multi-line failure is shown in full, not truncated to a summary."""
    lines = [f"traceback line {index:02d}: something went wrong" for index in range(20)]
    last_line = lines[-1]
    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        await _turn(
            app,
            pilot,
            [
                ToolUse("call-1", "bash", {"command": "run-job"}),
                ToolResult("call-1", "bash", "\n".join(lines), status="error"),
            ],
        )
        await scroll_end(app, pilot)
        rendered = screen_text(app)

    assert "traceback line 00" in rendered, (
        f"V7: the first error line was folded away:\n{rendered}"
    )
    assert last_line in rendered, (
        f"V7: the last error line ({last_line!r}) is not on screen; the long error "
        f"did not auto-expand:\n{rendered}"
    )


async def test_v8_a_real_file_write_renders_a_diff_on_screen(tmp_path: Path) -> None:
    """V8: a real `write_file` call paints the diff a reader expects."""
    target = tmp_path / DIFF_PATH
    target.write_text("alpha\nbravo\n", encoding="utf-8")

    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        await _turn(
            app,
            pilot,
            [
                ToolUse(
                    "call-1",
                    "write_file",
                    {"file_path": str(target), "content": "alpha\ncharlie\n"},
                ),
                Mutate(lambda: target.write_text("alpha\ncharlie\n", encoding="utf-8")),
                ToolResult("call-1", "write_file", "Updated file"),
            ],
        )
        # A single-tool group still folds; open it the way a reader would.
        groups = list(app.query("ToolGroupSummary"))
        if groups:
            await pilot.click(groups[0])
            await pilot.pause()
            await pilot.pause()
            await scroll_end(app, pilot)
        rendered = screen_text(app)

    assert DIFF_PATH in rendered, f"V8: the diff never names the file:\n{rendered}"
    assert re.search(r"^.*-\s+bravo", rendered, re.MULTILINE), (
        f"V8: the removed line is not marked:\n{rendered}"
    )
    assert re.search(r"^.*\+\s+charlie", rendered, re.MULTILINE), (
        f"V8: the added line is not marked:\n{rendered}"
    )
    assert "-1" in rendered, f"V8: the removal count is missing:\n{rendered}"
    assert "+1" in rendered, f"V8: the addition count is missing:\n{rendered}"


async def test_v10_the_activity_line_names_the_work_and_counts_elapsed() -> None:
    """V10: the verb, the elapsed counter, and the interrupt hint are real."""
    gate = asyncio.Event()
    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        agent = ScriptedAgent([Gate(gate), Sleep(0.2), Text("done")])
        await arm_turn(app, pilot, agent)
        assert _loading_text(app) == "", "V10: an idle app is already showing activity"

        await app._submit_input("go", "normal")
        await wait_running(app, pilot)
        await asyncio.sleep(1.2)
        await pilot.pause()
        running = _loading_text(app)

        gate.set()
        await wait_idle(app, pilot)
        idle = _loading_text(app)

    assert "Thinking" in running, f"V10: the activity verb is missing: {running!r}"
    assert "esc to interrupt" in running, (
        f"V10: the interrupt hint is missing: {running!r}"
    )
    assert re.search(r"\(\d+s", running), (
        f"V10: the elapsed counter did not advance: {running!r}"
    )
    assert idle == "", "V10: the activity line survived the end of the turn"


async def test_v11_markdown_renders_structure_not_source() -> None:
    """V11: headings, bullets, and emphasis render; the source syntax does not."""
    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        await _turn(
            app,
            pilot,
            [Text("# Findings\n\n- first point\n- second point\n\n**bold** claim\n")],
        )
        rendered = screen_text(app)

    assert "Findings" in rendered, f"V11: the heading never rendered:\n{rendered}"
    assert "# Findings" not in rendered, (
        f"V11: the heading marker was printed literally:\n{rendered}"
    )
    assert "first point" in rendered
    assert "second point" in rendered
    assert not re.search(r"^\s*-\s+first point", rendered, re.MULTILINE), (
        f"V11: list markers were printed literally:\n{rendered}"
    )
    assert "**bold**" not in rendered, (
        f"V11: emphasis source syntax survived rendering:\n{rendered}"
    )
    assert "bold" in rendered


async def test_f2_activity_appears_from_a_real_event_and_ends_with_it() -> None:
    """F2: no busy indicator without an event, none after the event settles."""
    gate = asyncio.Event()
    app = make_app()
    async with app.run_test(size=SIZE) as pilot:
        await pilot.pause()
        assert _loading_text(app) == "", "F2: a fresh app claims to be working"

        agent = ScriptedAgent([Gate(gate), Text("finished")])
        await arm_turn(app, pilot, agent)
        await app._submit_input("go", "normal")
        await wait_running(app, pilot)
        during = _loading_text(app)

        gate.set()
        await wait_idle(app, pilot)
        after = _loading_text(app)

    assert during != "", "F2: a real running turn showed no activity"
    assert after == "", "F2: the activity line outlived the event that caused it"
