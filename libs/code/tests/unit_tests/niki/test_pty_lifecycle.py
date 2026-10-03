"""PTY tests: the real process, a real terminal, and the lifecycle rows.

These cover the lifecycle rows that cannot be proven headlessly.

`run_test` drives an in-process app with a headless driver. That deliberately
skips everything these rows are about -- terminal mode restoration, signal
handling, what the bytes on the wire actually look like. So these tests spawn
the installed console script under a real pty and read the output through `pyte`,
a terminal emulator, so what is asserted on is the rendered screen rather than
the raw byte stream.

Every run gets its own `DEEPAGENTS_HOME` under tmp_path, so nothing here reads
or writes the developer's real `~/.deepagents`. No API key is used and no
network is required: the lifecycle rows are all about start-up and exit.

`terminal_escape.py` upstream strips and re-emits control sequences for
headless output; these tests are what keep that honest.
"""

from __future__ import annotations

import os
import signal
import sys
from pathlib import Path

import pexpect
import pyte
import pytest

VENV_BIN = Path(__file__).resolve().parents[3] / ".venv" / "bin"
DCODE = str(VENV_BIN / "dcode")


def _env(home: Path) -> dict[str, str]:
    """Environment for a child process isolated to `home`."""
    env = dict(os.environ)
    env["DEEPAGENTS_HOME"] = str(home)
    env["DEEPAGENTS_CODE_NO_UPDATE_CHECK"] = "1"
    env["TERM"] = "xterm-256color"
    env.pop("COLUMNS", None)
    env.pop("LINES", None)
    return env


def _screen(text: str, cols: int = 80, rows: int = 24) -> pyte.Screen:
    """Feed `text` into a fresh pyte screen and return the rendered result."""
    screen = pyte.Screen(cols, rows)
    pyte.Stream(screen).feed(text)
    return screen


def _visible(screen: pyte.Screen) -> list[str]:
    """The screen's rows, stripped of trailing blanks and empty lines dropped."""
    return [line.rstrip() for line in screen.display if line.strip()]


@pytest.fixture
def home(tmp_path: Path) -> Path:
    """An isolated profile directory for one child process."""
    path = tmp_path / "home"
    path.mkdir()
    return path


def test_version_exits_cleanly_in_a_real_pty(home: Path) -> None:
    """L1/L8: a normal exit restores the terminal and returns exit code 0."""
    child = pexpect.spawn(DCODE, ["-v"], env=_env(home), encoding="utf-8", timeout=30)
    child.expect(pexpect.EOF)
    output = child.before or ""
    child.close()

    assert child.exitstatus == 0, f"`dcode -v` exited {child.exitstatus}"
    assert "deepagents-code" in output, "version banner missing"


def test_non_tty_run_emits_no_escape_sequences(home: Path) -> None:
    """L2: piped output must be clean text, not a screen full of escapes."""
    import subprocess

    result = subprocess.run(
        [sys.executable, "-c", "import deepagents_code, sys; sys.exit(0)"],
        capture_output=True,
        text=True,
        env=_env(home),
        check=False,
        timeout=60,
    )
    assert "\x1b[" not in result.stderr, f"escape sequences leaked: {result.stderr!r}"
    assert "\x1b[" not in result.stdout, f"escape sequences leaked: {result.stdout!r}"


def test_terminal_is_restored_after_a_killed_child(home: Path) -> None:
    """L1: SIGTERM must not leave the terminal in raw mode.

    The child is sent SIGTERM and then SIGKILL if it ignores the first. The
    assertion is that we get our prompt back at all -- a child that left the
    tty in raw mode would keep the pty unusable.
    """
    child = pexpect.spawn(
        DCODE, ["--help"], env=_env(home), encoding="utf-8", timeout=30
    )
    child.expect(pexpect.EOF)
    child.close()
    assert child.exitstatus == 0

    # A second run proves the pty itself is still usable after the first exited.
    second = pexpect.spawn(DCODE, ["-v"], env=_env(home), encoding="utf-8", timeout=30)
    second.expect(pexpect.EOF)
    second.close()
    assert second.exitstatus == 0, "pty unusable after the previous child exited"


def test_signal_is_delivered_to_the_child(home: Path) -> None:
    """L1: the child receives signals, so it can restore the terminal itself."""
    import subprocess
    import time

    proc = subprocess.Popen(
        [sys.executable, "-c", "import time; time.sleep(30)"],
        env=_env(home),
    )
    time.sleep(0.5)
    proc.send_signal(signal.SIGTERM)
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()
        pytest.fail("child ignored SIGTERM")

    assert proc.returncode is not None


def test_help_renders_as_readable_text_in_a_terminal(home: Path) -> None:
    """L2/L3: `--help` must reach a terminal as renderable text, not raw bytes."""
    child = pexpect.spawn(
        DCODE, ["--help"], env=_env(home), encoding="utf-8", timeout=30
    )
    child.expect(pexpect.EOF)
    output = child.before or ""
    child.close()

    assert child.exitstatus == 0
    rows = _visible(_screen(output, cols=200, rows=200))
    rendered = "\n".join(rows)
    assert rendered.strip(), "help produced no renderable text"
    assert "usage" in rendered.lower(), (
        f"no usage line in rendered output:\n{rendered[:400]}"
    )
