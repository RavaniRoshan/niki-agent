"""V15: screen-reader mode is linear -- no spinner, no escape sequences.

A screen reader flattens the terminal into a stream of lines. Anything that
repaints a line in place is either repeated or lost, so reader mode must not
construct the live spinner and must not emit cursor movement. The PTY test
covers the other half of the contract: a bare interactive launch has no
linear conversation to follow, so it explains how to run one turn and exits
without mounting the TUI.
"""

from __future__ import annotations

import io
import os
import sys
from pathlib import Path
from typing import TYPE_CHECKING
from unittest.mock import AsyncMock, MagicMock, patch

import pexpect
import pytest
from langchain_core.messages import AIMessage

from niki_agent.client.non_interactive import run_non_interactive
from niki_agent.config import ModelResult, runtime_state
from niki_agent.niki.accessibility import (
    NIKI_SCREEN_READER_ENV,
    screen_reader_enabled,
)

if TYPE_CHECKING:
    from collections.abc import AsyncIterator, Sequence

VENV_BIN = Path(__file__).resolve().parents[3] / ".venv" / "bin"
NIKI = str(VENV_BIN / "niki")


@pytest.mark.parametrize("value", ["1", "true", "TRUE", "yes", "on", " On "])
def test_truthy_spellings_enable_reader_mode(
    monkeypatch: pytest.MonkeyPatch, value: str
) -> None:
    monkeypatch.setenv(NIKI_SCREEN_READER_ENV, value)
    assert screen_reader_enabled() is True


@pytest.mark.parametrize("value", ["", "0", "false", "no", "none", "off", "2"])
def test_falsy_spellings_leave_reader_mode_off(
    monkeypatch: pytest.MonkeyPatch, value: str
) -> None:
    monkeypatch.setenv(NIKI_SCREEN_READER_ENV, value)
    assert screen_reader_enabled() is False


def test_unset_means_off(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv(NIKI_SCREEN_READER_ENV, raising=False)
    assert screen_reader_enabled() is False


async def _stream(items: Sequence[object]) -> AsyncIterator[object]:  # noqa: RUF029
    """Yield the scripted stream chunks a fake agent produces."""
    for item in items:
        yield item


async def test_reader_mode_output_is_plain_text_and_skips_the_spinner(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The answer still prints, but nothing paints over a line to show it."""
    monkeypatch.setenv(NIKI_SCREEN_READER_ENV, "1")
    ai_msg = MagicMock(spec=AIMessage)
    ai_msg.content_blocks = [{"type": "text", "text": "Hello from agent"}]
    stream_chunks = [("", "messages", (ai_msg, {}))]

    mock_agent = MagicMock()
    mock_agent.astream = MagicMock(return_value=_stream(stream_chunks))
    mock_server_proc = MagicMock()

    stdout_buf = io.StringIO()
    stderr_buf = io.StringIO()

    with (
        patch(
            "niki_agent.client.non_interactive.create_model",
            return_value=ModelResult(
                model=MagicMock(),
                model_name="test-model",
                provider="test",
            ),
        ),
        patch(
            "niki_agent.client.non_interactive.generate_thread_id",
            return_value="test-thread",
        ),
        patch(
            "niki_agent.client.non_interactive._resolve_shell_allow_list",
        ) as mock_settings,
        patch(
            "niki_agent.client.non_interactive.build_langsmith_thread_url",
            return_value=None,
        ),
        patch(
            "niki_agent.client.launch.server_manager.start_server_and_get_agent",
            new_callable=AsyncMock,
            return_value=(mock_agent, mock_server_proc, None),
        ),
        patch(
            "niki_agent.client.non_interactive._ConsoleSpinner",
        ) as mock_spinner,
        patch.object(sys, "stdout", stdout_buf),
        patch.object(sys, "stderr", stderr_buf),
        patch.dict(os.environ, {"COLUMNS": "80", "LINES": "24"}),
    ):
        mock_settings.return_value = None
        mock_settings.has_tavily = False
        runtime_state.model_name = None

        await run_non_interactive(message="test")

    stdout_text = stdout_buf.getvalue()
    assert "Hello from agent" in stdout_text
    assert "\x1b[" not in stdout_text, f"escape sequences leaked: {stdout_text!r}"
    mock_spinner.assert_not_called()


def _env(home: Path) -> dict[str, str]:
    env = dict(os.environ)
    env["NIKI_HOME"] = str(home)
    env[NIKI_SCREEN_READER_ENV] = "1"
    env["NIKI_NO_UPDATE_CHECK"] = "1"
    env["TERM"] = "xterm-256color"
    env.pop("COLUMNS", None)
    env.pop("LINES", None)
    return env


def test_bare_launch_explains_linear_mode_and_exits(tmp_path: Path) -> None:
    """No prompt, no TUI: one line of guidance and a non-zero exit."""
    home = tmp_path / "home"
    home.mkdir()
    child = pexpect.spawn(NIKI, [], env=_env(home), encoding="utf-8", timeout=60)
    child.expect(pexpect.EOF)
    output = child.before or ""
    child.close()

    assert child.exitstatus == 2, f"bare reader-mode launch exited {child.exitstatus}"
    assert "Screen-reader mode is linear" in output
    assert 'niki -n "..."' in output
    assert "\x1b[" not in output, f"escape sequences leaked: {output!r}"
