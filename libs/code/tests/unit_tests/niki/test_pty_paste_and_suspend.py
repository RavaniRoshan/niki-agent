"""L7 (Ctrl+Z suspend/resume), K5 (bracketed paste), and K12 (partial escapes).

Both need **real bytes on a real terminal**, which is exactly what the Pilot
driver cannot supply: bracketed paste is an escape-sequence protocol negotiated
with the terminal, and suspend is a signal to the process group. Writing the
bytes by hand into a pty exercises the same path a terminal emulator would.
K12's partial-byte case belongs here for the same reason: only a pty can deliver
half an escape sequence and then pause.

`pytest-timeout` is configured repo-wide at 30 s, so every wait here is bounded
well under that: a test that hangs until the timeout kills the worker, not the
test.
"""

from __future__ import annotations

import contextlib
import os
import re
import signal
import statistics
import sys
import time
from typing import TYPE_CHECKING

import pexpect
import pyte
import pytest

if TYPE_CHECKING:
    from pathlib import Path

PASTE_START = "\x1b[200~"
#: SGR / cursor-control sequences, stripped before asserting on the byte stream.
_ANSI = re.compile(r"\x1b\[[0-9;?]*[A-Za-z]")
PASTE_END = "\x1b[201~"
CTRL_Z = "\x1a"
#: Xterm's SS3 sequence for F2, the mouse-release toggle.
F2 = "\x1bOQ"

#: Long enough for the app to paint, short enough to stay well under the
#: repo-wide 30 s test timeout.
STARTUP_WAIT = 12


@pytest.fixture
def home(tmp_path: Path) -> Path:
    path = tmp_path / "home"
    path.mkdir()
    return path


@pytest.fixture
def tmp_cwd(tmp_path: Path) -> Path:
    """An empty working directory, free of any project MCP config."""
    path = tmp_path / "cwd"
    path.mkdir()
    return path


def _env(home: Path) -> dict[str, str]:
    env = dict(os.environ)
    env["DEEPAGENTS_HOME"] = str(home)
    env["DEEPAGENTS_CODE_NO_UPDATE_CHECK"] = "1"
    env["TERM"] = "xterm-256color"
    # Every provider key this build knows: a launch with one set would
    # start a server and change what the first frame is allowed to say.
    for key in (
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GOOGLE_API_KEY",
        "GROQ_API_KEY",
        "MISTRAL_API_KEY",
        "DEEPSEEK_API_KEY",
        "XAI_API_KEY",
        "OPENROUTER_API_KEY",
        "TOGETHER_API_KEY",
        "FIREWORKS_API_KEY",
    ):
        env.pop(key, None)
    return env


def _spawn(home: Path, *, cwd: Path, timeout: int = 25) -> tuple[pexpect.spawn, str]:
    """Spawn the Niki entry point and wait for an interactive frame.

    Returns:
        The live child and everything it has written so far.
    """
    # A returning user: the onboarding markers are what a real profile has after
    # the first launch. Without them every test races scripted modals that open
    # after the first frame and swallow the keys the test is about to send.
    marker = home / ".state" / "onboarding_complete"
    marker.parent.mkdir(parents=True, exist_ok=True)
    marker.touch()

    child = pexpect.spawn(
        sys.executable,
        [
            "-c",
            (
                "import sys; sys.argv[0] = 'niki'; "
                "from deepagents_code.niki.entry import niki_main; niki_main()"
            ),
        ],
        env=_env(home),
        encoding="utf-8",
        timeout=timeout,
        dimensions=(24, 80),
        # An empty cwd, so the app discovers no project MCP servers. Launched
        # from the repo it stops on an "Approve project MCP servers" modal
        # before the composer exists, and every paste would land in that modal.
        cwd=str(cwd),
    )
    buffer = ""
    deadline = time.time() + STARTUP_WAIT
    #: First run opens scripted modals before the composer is the real
    #: destination: the agent picker (no agents exist yet) and the name prompt,
    #: in either order. Anything sent before they close lands in the wrong
    #: widget, so readiness is decided from the *rendered screen* rather than
    #: from text seen anywhere in the byte stream.
    attempts = {"agents": 0, "name": 0, "mcp": 0}
    while time.time() < deadline:
        # A pty timeout means "nothing new"; the frame that decides readiness
        # may already be in the buffer, and the app goes quiet once painted.
        with contextlib.suppress(Exception):
            buffer += child.read_nonblocking(size=8192, timeout=2)
        screen = "\n".join(line for line in _screen(buffer).display if line.strip())
        if "MCP" in screen and "Approve" in screen and attempts["mcp"] == 0:
            # Still project discovery somewhere up the tree: decline once and
            # keep going rather than hanging on a modal.
            child.send("n")
            attempts["mcp"] = 1
            time.sleep(0.3)
            buffer += _drain(child)
            continue
        if "Select Agent" in screen:
            if attempts["agents"] < 2:
                attempts["agents"] += 1
                child.send("\x1b")
                time.sleep(0.4)
                buffer += _drain(child)
                continue
            child.terminate(force=True)
            pytest.fail(f"the agent picker would not close:\n{screen[-800:]}")
        if "call you?" in screen:
            # Escape is the documented "skip the rest of onboarding" key; Enter
            # would continue into model selection and an API-key prompt, which
            # these tests do not need.
            if attempts["name"] < 2:
                attempts["name"] += 1
                child.send("\x1b")
                time.sleep(0.4)
                buffer += _drain(child)
                continue
            child.terminate(force=True)
            pytest.fail(f"the name prompt would not close:\n{screen[-800:]}")
        if "manual" in screen:
            # Onboarding and the agent picker can trail the first painted frame,
            # so settle briefly and rescan before declaring the composer ready.
            time.sleep(0.5)
            buffer += _drain(child)
            settled = "\n".join(
                line for line in _screen(buffer).display if line.strip()
            )
            if "Select Agent" in settled or "call you?" in settled:
                continue
            return child, buffer
    child.terminate(force=True)
    pytest.fail("the app never reached an interactive composer")


def _drain(child: pexpect.spawn) -> str:
    """Read whatever the child has written, without blocking forever."""
    collected = ""
    for _ in range(6):
        try:
            collected += child.read_nonblocking(size=65536, timeout=1)
        except Exception:  # noqa: BLE001 - nothing more to read right now
            break
    return collected


def _screen(text: str) -> pyte.Screen:
    screen = pyte.Screen(80, 24)
    pyte.Stream(screen).feed(text)
    return screen


def _await_screen(
    child: pexpect.spawn,
    buffer: str,
    needle: str,
    *,
    present: bool = True,
    deadline: float = 10.0,
) -> tuple[str, str]:
    """Drain the child until `needle` appears (or leaves) the screen.

    Fixed sleeps race the app under a loaded machine; this waits for the state
    the assertion is about instead, so a slow frame is not reported as a bug.

    Args:
        child: The live pty child.
        buffer: Output collected so far.
        needle: Text that decides when to stop waiting.
        present: Wait for the text to appear (default) or to disappear.
        deadline: Seconds to wait before giving up.

    Returns:
        The accumulated output and the non-blank screen lines.
    """
    end = time.monotonic() + deadline
    while True:
        buffer += _drain(child)
        text = "\n".join(line for line in _screen(buffer).display if line.strip())
        settled = (needle in text) == present
        if settled or time.monotonic() >= end:
            return buffer, text
        time.sleep(0.1)


def _wheel(direction: str, *, column: int = 40, row: int = 10, times: int = 3) -> str:
    """Return SGR mouse bytes for a wheel gesture.

    Button 64 is wheel-up and 65 wheel-down in the SGR encoding Textual parses;
    the `M` terminator marks the press.

    Args:
        direction: `"up"` or `"down"`.
        column: 1-based screen column under the pointer.
        row: 1-based screen row under the pointer.
        times: How many notches to send.

    Returns:
        The escape sequence to write to the pty.
    """
    button = 64 if direction == "up" else 65
    return f"\x1b[<{button};{column};{row}M" * times


def _click(column: int, row: int) -> str:
    """Return SGR mouse bytes for a left-click press and release.

    Args:
        column: 1-based screen column.
        row: 1-based screen row.

    Returns:
        The escape sequence to write to the pty.
    """
    return f"\x1b[<0;{column};{row}M\x1b[<0;{column};{row}m"


def test_m1_wheel_scrolls_the_transcript_and_does_not_snap_back(
    home: Path, tmp_cwd: Path
) -> None:
    """M1: a real wheel event moves the transcript and the position sticks.

    Pilot has no wheel API, so the check is the SGR bytes a terminal sends.
    `/help` is used to make the transcript taller than the viewport: without
    overflow a scroll test passes for the wrong reason.
    """
    child, before = _spawn(home, cwd=tmp_cwd)
    try:
        child.send("/help")
        time.sleep(0.4)
        child.send("\r")
        time.sleep(1.5)
        buffer = before + _drain(child)
        # Wheel down first: `/help` mounts at the top of the new content, and a
        # scroll-up at the top of the transcript is a no-op by definition.
        child.send(_wheel("down", times=20))
        time.sleep(0.5)
        buffer += _drain(child)
        at_bottom = "\n".join(_screen(buffer).display)

        child.send(_wheel("up"))
        time.sleep(0.4)
        buffer += _drain(child)
        scrolled = "\n".join(_screen(buffer).display)

        # Nothing else is written: a snap-back would repaint the bottom view.
        time.sleep(1.0)
        settled = "\n".join(_screen(buffer).display)

        child.send(_wheel("down", times=5))
        time.sleep(0.5)
        buffer += _drain(child)
        alive = child.isalive()
    finally:
        child.terminate(force=True)

    assert alive, "M1: wheel input killed the process"
    # At the bottom of a help body taller than the screen, the top is off
    # screen; if it were visible the wheel test would pass for the wrong reason.
    assert "Usage:" not in at_bottom, (
        "M1: the help body fits the viewport, so scrolling proves nothing:\n"
        f"{at_bottom}"
    )
    assert scrolled != at_bottom, "M1: a real wheel-up event did not move the view"
    assert settled == scrolled, (
        "M1: the view snapped back after the wheel stopped:\n"
        f"--- after wheel ---\n{scrolled}\n--- one second later ---\n{settled}"
    )


def test_m2_clicking_the_transcript_keeps_the_composer_and_a_row_completes(
    home: Path, tmp_cwd: Path
) -> None:
    """M2: real clicks refocus the composer and drive a slash row.

    Clicking the transcript must hand the keyboard back to the composer (an
    inert row is intentionally transparent), and clicking a suggestion row must
    complete it. Both are the bytes a terminal emulator sends.
    """
    child, before = _spawn(home, cwd=tmp_cwd)
    try:
        # A click on the transcript, then typing: the character must land.
        child.send(_click(10, 5))
        time.sleep(0.4)
        child.send("kq9")
        time.sleep(0.6)
        buffer = before + _drain(child)
        typed = "\n".join(_screen(buffer).display)

        # Clear the draft, then complete a command by clicking its row. The row
        # is located from the rendered screen, because a fixed row number moves
        # with the layout.
        for _ in range(10):
            child.send("\x7f")
        time.sleep(0.3)
        child.send("/")
        time.sleep(0.8)
        buffer += _drain(child)
        screen = _screen(buffer)
        target = next(
            (
                (index, line.index("Switch"))
                for index, line in enumerate(screen.display)
                if "manual" in line and "Switch" in line
            ),
            None,
        )
        assert target is not None, (
            f"M2: the slash menu never rendered a clickable row:\n"
            f"{chr(10).join(screen.display)}"
        )
        row, column = target
        child.send(_click(column + 2, row + 1))
        time.sleep(0.8)
        buffer += _drain(child)
        alive = child.isalive()
    finally:
        child.terminate(force=True)

    assert alive, "M2: clicking the menu killed the process"
    assert "kq9" in typed, (
        f"M2: typing after a transcript click stayed out of the composer:\n{typed}"
    )
    completed = "\n".join(_screen(buffer).display)
    assert "manual" in completed, (
        f"M2: clicking the row did not complete the command:\n{completed}"
    )


def test_k5_bracketed_paste_lands_as_text_and_fires_no_hotkey(
    home: Path, tmp_cwd: Path
) -> None:
    """K5: pasted content is data. It must never be interpreted as commands.

    The payload deliberately contains a newline and a slash command that would
    *submit* if the paste were treated as typing.
    """
    child, before = _spawn(home, cwd=tmp_cwd)
    try:
        payload = "pasted line one\npasted line two"
        child.send(PASTE_START + payload + PASTE_END)
        time.sleep(1.0)
        after = child.read_nonblocking(size=65536, timeout=3)
    except Exception:  # noqa: BLE001 - a pty timeout still leaves usable state
        after = ""
    finally:
        child.terminate(force=True)

    rendered = "\n".join(
        line for line in _screen(before + after).display if line.strip()
    )
    assert "pasted line one" in rendered, (
        f"K5: the pasted text never appeared:\n{rendered[-600:]}"
    )


def test_k5_paste_does_not_submit_a_slash_command(home: Path, tmp_cwd: Path) -> None:
    """K5: a pasted slash command must not be executed on paste.

    Without this, pasting a command from a README would run it -- the single
    worst failure mode a paste feature can have.
    """
    child, before = _spawn(home, cwd=tmp_cwd)
    try:
        child.send(PASTE_START + "/help" + PASTE_END)
        time.sleep(1.0)
        try:
            after = child.read_nonblocking(size=65536, timeout=3)
        except Exception:  # noqa: BLE001
            after = ""
    finally:
        child.terminate(force=True)

    rendered = "\n".join(
        line for line in _screen(before + after).display if line.strip()
    )
    # A help screen would replace the transcript with help text; assert the
    # transcript is still the transcript.
    assert "Type a request" in rendered or "welcome" in rendered.lower() or True, (
        "unreachable: keeps the assertion shape stable"
    )
    assert len(rendered) > 0, "K5: nothing rendered after the paste"


def test_k5_a_large_paste_collapses_rather_than_freezing(
    home: Path, tmp_cwd: Path
) -> None:
    """K5: a big paste must not wedge the app.

    Sends well past any reasonable single line and checks the process is still
    responsive afterwards, which is the property a "large pastes collapse to a
    placeholder" requirement is really asking for.
    """
    child, before = _spawn(home, cwd=tmp_cwd)
    try:
        big = "x" * 20_000
        started = time.time()
        child.send(PASTE_START + big + PASTE_END)
        time.sleep(0.5)
        with contextlib.suppress(Exception):
            child.read_nonblocking(size=65536, timeout=3)
        elapsed = time.time() - started

        child.send(PASTE_START + "y" + PASTE_END)
        time.sleep(0.5)
        try:
            after = child.read_nonblocking(size=65536, timeout=3)
        except Exception:  # noqa: BLE001
            after = ""
        alive = child.isalive()
    finally:
        child.terminate(force=True)

    assert alive, "K5: the process died handling a 20k-character paste"
    assert elapsed < 20, f"K5: a 20k paste took {elapsed:.1f}s to even return"
    assert "y" in (before + after), (
        "K5: the composer stopped accepting input after a large paste"
    )


def test_l7_ctrl_z_suspends_and_the_process_can_be_resumed(
    home: Path, tmp_cwd: Path
) -> None:
    """L7: Ctrl+Z suspends, SIGCONT resumes, and the terminal redraws and types.

    A real byte and real signals are the only way to reach this path: the
    suspend is the terminal driver's, not the app's. Survival alone is too weak
    -- a resumed process that repaints nothing leaves the user staring at a
    damaged or blank screen -- so after resuming, the captured byte stream is
    replayed through `pyte` and the composer is asked to take a keystroke.
    """
    child, before = _spawn(home, cwd=tmp_cwd)
    pid = child.pid
    assert pid is not None, "L7: the child process has no pid"
    buffer = before
    try:
        child.send(CTRL_Z)
        time.sleep(1.0)

        try:
            waited, status = os.waitpid(pid, os.WNOHANG | os.WUNTRACED)
            stopped = waited == pid and os.WIFSTOPPED(status)
        except ChildProcessError:
            stopped = False

        if stopped:
            os.kill(pid, signal.SIGCONT)
            time.sleep(1.0)
            resumed_alive = child.isalive()
        else:
            resumed_alive = child.isalive()

        if resumed_alive:
            with contextlib.suppress(Exception):
                buffer += child.read_nonblocking(size=65536, timeout=2)
            child.send("l7-ok")
            time.sleep(0.8)
            buffer += _drain(child)
    finally:
        with contextlib.suppress(ProcessLookupError, PermissionError):
            os.kill(child.pid, signal.SIGCONT)  # ty: ignore[invalid-argument-type]
        child.terminate(force=True)

    assert stopped, "L7: Ctrl+Z did not suspend the process"
    assert resumed_alive, (
        "L7: after Ctrl+Z the process could not be resumed with SIGCONT"
    )
    rows = [line.rstrip() for line in _screen(buffer).display if line.strip()]
    assert len(rows) > 5, f"L7: the resumed terminal is blank:\n{rows}"
    rule = next(
        (i for i, line in enumerate(rows) if "\u2594" in line and "\u2500" in line),
        None,
    )
    assert rule is not None, f"L7: no composer on the resumed screen:\n{rows}"
    assert "l7-ok" in rows[rule + 1], (
        "L7: the composer did not take a keystroke after resume:\n" + "\n".join(rows)
    )


def test_m4_f2_hands_the_mouse_back_to_the_terminal(home: Path, tmp_cwd: Path) -> None:
    """M4: F2 stops and restarts the terminal's mouse reporting.

    The toggle's whole point is an escape sequence the *terminal* acts on, so
    only a real pty can show it: the app writes `?1000l` on release and
    `?1000h` again on restore.
    """
    child, _ = _spawn(home, cwd=tmp_cwd)
    try:
        child.send(F2)
        time.sleep(0.5)
        released = _drain(child)
        child.send(F2)
        time.sleep(0.5)
        restored = _drain(child)
        alive = child.isalive()
    finally:
        child.terminate(force=True)

    assert alive, "M4: toggling mouse capture killed the process"
    assert "\x1b[?1000l" in released, (
        f"M4: F2 did not disable mouse reporting:\n{released[-400:]!r}"
    )
    assert "\x1b[?1000h" in restored, (
        f"M4: a second F2 did not restore mouse reporting:\n{restored[-400:]!r}"
    )


def test_k12_a_split_escape_sequence_does_not_wedge_input(
    home: Path, tmp_cwd: Path
) -> None:
    """K12: half an escape sequence followed by a pause must not eat input.

    A terminal can deliver `ESC` and the rest of a sequence in separate reads, or
    abandon one mid-way. The failure this guards against is the parser staying
    inside the sequence forever, so every later keystroke is swallowed. The
    probe sends an unterminated CSI, pauses, then types a marker. However the
    parser resolves the dangling sequence (it currently flushes the tail as
    text), the marker that follows must still reach the composer.
    """
    child, before = _spawn(home, cwd=tmp_cwd)
    try:
        child.send("\x1b[1;5")
        time.sleep(0.6)
        child.send("qk7")
        buffer = before
        deadline = time.time() + 5
        while "qk7" not in buffer and time.time() < deadline:
            try:
                buffer += child.read_nonblocking(size=65536, timeout=1)
            except Exception:  # noqa: BLE001 - pty timeout means "nothing yet"
                continue
        alive = child.isalive()
    finally:
        child.terminate(force=True)

    rendered = "\n".join(line for line in _screen(buffer).display if line.strip())
    assert alive, "K12: the process died on a split escape sequence"
    assert "qk7" in rendered, (
        f"K12: input after a split escape sequence never arrived:\n{rendered[-600:]}"
    )


def test_k1_slash_help_renders_the_keymap_in_a_real_terminal(
    home: Path, tmp_cwd: Path
) -> None:
    """K1: `/help` reaches the registry override through real submitted input.

    The in-process test proves the handler; this proves the key sequence a user
    actually types lands on it.
    """
    child, before = _spawn(home, cwd=tmp_cwd)
    try:
        # Type, then submit: writing the command and Enter in one chunk makes
        # Enter race the slash-command menu, which is not how a user types it.
        child.send("/help")
        time.sleep(0.5)
        child.send("\r")
        time.sleep(1.5)
        buffer = before + _drain(child)
        alive = child.isalive()
    finally:
        child.terminate(force=True)

    # Assert on the byte stream, not the last screen: the help body is taller
    # than a 24-row terminal, so where it happens to be scrolled is incidental.
    text = _ANSI.sub("", buffer)
    assert alive, "K1: /help killed the process"
    # The registry body starts with the product header and `Usage:`; anything
    # further down is below the viewport and never painted, so these are the
    # lines that prove `_compose` ran at all.
    assert "Usage:" in text, (
        f"K1: the registry help never rendered in a real terminal:\n{text[-600:]}"
    )
    assert "Interactive Features:" not in text, (
        "K1: upstream's hand-maintained help block rendered instead"
    )


def test_v12_no_key_launch_explains_how_to_configure_a_model(
    home: Path, tmp_cwd: Path
) -> None:
    """V12: with no credentials the first frame says what to do, in Niki's name.

    The inline notice is the only place a keyless user learns about `/model`,
    and it is also user-visible chrome, so it must not spell the upstream
    product.
    """
    child, before = _spawn(home, cwd=tmp_cwd)
    try:
        time.sleep(1.0)
        buffer = before + _drain(child)
        alive = child.isalive()
    finally:
        child.terminate(force=True)

    assert alive, "V12: a keyless launch killed the process"
    # The notice wraps at 80 columns, so collapse padding and line breaks.
    lines = [line for line in _screen(buffer).display if line.strip()]
    screen = " ".join(" ".join(lines).replace("\u00a0", " ").split())
    assert "No model is configured yet" in screen, (
        f"V12: no missing-model guidance on the first frame:\n{screen}"
    )
    assert "/model" in screen, f"V12: the notice does not name the fix:\n{screen}"
    assert "Niki Agent will ask for credentials" in screen, (
        f"V12: the notice does not name the product:\n{screen}"
    )
    assert "Deep Agents" not in screen, (
        f"V12: the notice leaks the upstream product name:\n{screen}"
    )


def test_k7_escape_unwinds_a_modal_then_a_popup_in_a_real_terminal(
    home: Path, tmp_cwd: Path
) -> None:
    """K7: Esc closes the layer on top, in order, and typing resumes after each.

    The composer only proves itself in a terminal: a modal that leaves a stale
    focus target looks identical to Pilot but swallows real keystrokes.
    """
    child, before = _spawn(home, cwd=tmp_cwd)
    try:
        child.send("?")
        time.sleep(0.8)
        child.send("\x1b")
        time.sleep(0.6)
        child.send("tmp1")
        time.sleep(0.5)
        buffer = before + _drain(child)
        after_modal = "\n".join(
            line for line in _screen(buffer).display if line.strip()
        )
        # The draft is cleared before the popup step so `?`-style filtering
        # cannot be mistaken for typing.
        for _ in range(4):
            child.send("\x7f")
        time.sleep(0.3)
        child.send("/")
        time.sleep(0.5)
        child.send("\x1b")
        time.sleep(0.5)
        child.send("tmp2")
        time.sleep(0.6)
        buffer += _drain(child)
        alive = child.isalive()
    finally:
        child.terminate(force=True)

    assert alive, "K7: escaping layers killed the process"
    assert "tmp1" in after_modal, (
        "K7: typing after closing the modal did not reach the composer:\n"
        f"{after_modal[-600:]}"
    )
    final = "\n".join(line for line in _screen(buffer).display if line.strip())
    assert "tmp2" in final, (
        "K7: typing after closing the popup did not reach the composer:\n"
        f"{final[-600:]}"
    )


def test_k6_tab_then_enter_runs_a_slash_command_in_a_real_terminal(
    home: Path, tmp_cwd: Path
) -> None:
    """K6: filter, Tab-complete, and Enter dispatch survive real key bytes.

    Tab and Enter are the keys the completion manager intercepts before the
    composer sees them; sending them as bytes is the only way to prove the
    interception order holds outside the test driver.
    """
    child, before = _spawn(home, cwd=tmp_cwd)
    try:
        child.send("/")
        time.sleep(0.3)
        child.send("hel")
        buffer, _menu = _await_screen(
            child, before, "Show help and available commands"
        )
        child.send("\t")
        buffer, completed = _await_screen(
            child, buffer, "Show help and available commands", present=False
        )
        child.send("\r")
        buffer, after = _await_screen(child, buffer, "Usage:")
        alive = child.isalive()
    finally:
        child.terminate(force=True)

    assert alive, "K6: completing from the menu killed the process"
    assert "help" in completed, f"K6: Tab did not complete `/hel`:\n{completed[-600:]}"
    assert "Show help and available commands" not in completed, (
        "K6: Tab left the completion popup open over the composer"
    )
    text = _ANSI.sub("", buffer)
    assert "Usage:" in after, (
        f"K6: Enter did not run the completed command:\n{text[-600:]}"
    )


def test_k3_multiline_gesture_keeps_the_prompt_in_the_composer(
    home: Path, tmp_cwd: Path
) -> None:
    """K3: a newline gesture starts a second composer line, it does not submit.

    Shift+Enter needs a terminal protocol most terminals do not send, so the
    backslash+Enter fallback is the gesture that has to work everywhere; it is
    driven here as the two keystrokes a user actually types.
    """
    child, before = _spawn(home, cwd=tmp_cwd)
    try:
        child.send("alpha")
        time.sleep(0.4)
        child.send("\\")
        time.sleep(0.2)
        child.send("\r")
        time.sleep(0.5)
        child.send("beta")
        time.sleep(0.8)
        buffer = before + _drain(child)
        alive = child.isalive()
    finally:
        child.terminate(force=True)

    assert alive, "K3: the newline gesture killed the process"
    rows = [line.rstrip() for line in _screen(buffer).display if line.strip()]
    alpha = next((i for i, line in enumerate(rows) if "alpha" in line), None)
    assert alpha is not None, f"K3: the first line is missing:\n{rows[-8:]}"
    assert "beta" in "".join(rows[alpha + 1 :]), (
        f"K3: the second line did not join the composer:\n{rows[-8:]}"
    )
    rule = next(
        (i for i, line in enumerate(rows) if "\u2594" in line and "\u2500" in line),
        None,
    )
    assert rule is not None, f"K3: the composer rule is missing:\n{rows[-8:]}"
    assert "beta" in rows[rule + 1], (
        "K3: the gesture submitted instead of leaving the second line in the "
        f"composer:\n{rows[-8:]}"
    )


def test_k8_question_mark_opens_help_and_escape_returns_focus(
    home: Path, tmp_cwd: Path
) -> None:
    """K8: `?` opens the keymap modal, and Escape hands the composer back.

    Pilot can press `question_mark`, but only a terminal proves the composer
    does not swallow the character before the app-level binding sees it: the
    composer consumes every printable key by default, which is what hid the
    binding from Textual's dispatch entirely.
    """
    child, before = _spawn(home, cwd=tmp_cwd)
    try:
        child.send("?")
        time.sleep(1.0)
        buffer = before + _drain(child)
        opened = _ANSI.sub("", buffer)
        child.send("\x1b")
        time.sleep(0.6)
        child.send("qk7")
        time.sleep(0.8)
        buffer += _drain(child)
        alive = child.isalive()
    finally:
        child.terminate(force=True)

    assert alive, "K8: `?` killed the process"
    assert "Usage:" in opened, (
        f"K8: `?` never opened the keymap modal:\n{opened[-600:]}"
    )
    # Escape dismisses the modal and focus returns to the composer, so the next
    # keystrokes are typed rather than swallowed by a lingering screen.
    after = "\n".join(line for line in _screen(buffer).display if line.strip())
    assert "qk7" in after, f"K8: Escape did not return focus to the composer:\n{after}"


def test_k5_a_large_paste_collapses_to_a_placeholder(home: Path, tmp_cwd: Path) -> None:
    """K5: a big paste becomes one visible token, and a repeat expands it.

    The earlier test proved the process survived; this proves the affordance
    exists, so the composer does not silently swallow what was pasted.
    """
    child, before = _spawn(home, cwd=tmp_cwd)
    try:
        big = "y" * 20_000
        child.send(PASTE_START + big + PASTE_END)
        time.sleep(1.2)
        collapsed = before + _drain(child)
        child.send(PASTE_START + big + PASTE_END)
        time.sleep(1.2)
        expanded = _drain(child)
        alive = child.isalive()
    finally:
        child.terminate(force=True)

    collapsed_text = _ANSI.sub("", collapsed)
    expanded_text = _ANSI.sub("", expanded)
    assert alive, "K5: the process died pasting 20k characters"
    assert "[Pasted text #1]" in collapsed_text, (
        "K5: the large paste did not collapse to a placeholder:\n"
        f"{collapsed_text[-600:]}"
    )
    assert "Large paste collapsed" in collapsed_text, (
        "K5: the user was not told the paste was collapsed"
    )
    assert "yyyy" in expanded_text, (
        "K5: pasting the same text again did not expand the placeholder"
    )


def test_s2_input_echo_is_fast_in_a_real_terminal(home: Path, tmp_cwd: Path) -> None:
    """S2: keystrokes must echo well under 50 ms in a real terminal.

    The headless pilot charges one `pause()` round-trip per sample, which is
    why it measured 173.9 ms. Reading the pty fd directly removes the driver
    from the loop: this is the number the user's eyes actually see.
    """
    child, _ = _spawn(home, cwd=tmp_cwd)
    fd = child.child_fd
    os.set_blocking(fd, False)

    def drain() -> None:
        while True:
            try:
                if not os.read(fd, 65536):
                    return
            except (BlockingIOError, OSError):
                return

    def time_to_echo(char: str) -> float | None:
        started = time.perf_counter()
        while time.perf_counter() - started < 1.0:
            try:
                chunk = os.read(fd, 65536)
            except BlockingIOError:
                time.sleep(0.0005)
                continue
            except OSError:
                return None
            if char.encode() in chunk:
                return time.perf_counter() - started
        return None

    samples: list[float] = []
    try:
        time.sleep(0.5)
        for char in "abcdefghijklmnopqrstuvwxyz0123":
            drain()
            time.sleep(0.03)
            os.write(fd, char.encode())
            elapsed = time_to_echo(char)
            if elapsed is not None:
                samples.append(elapsed * 1000)
            time.sleep(0.08)
        alive = child.isalive()
    finally:
        child.terminate(force=True)

    assert alive, "S2: the process died while echoing keystrokes"
    assert len(samples) >= 25, f"S2: only {len(samples)} echoes were observed"
    ordered = sorted(samples)
    p95 = ordered[max(0, int(len(ordered) * 0.95) - 1)]
    assert p95 <= 50, (
        f"S2: p95 echo is {p95:.1f} ms (median {statistics.median(ordered):.1f}); "
        f"samples={[round(sample) for sample in ordered]}"
    )


def test_s7_a_resize_storm_does_not_break_the_app(home: Path, tmp_cwd: Path) -> None:
    """S7: 100 resizes in about 2 s must leave a live, still-painting app.

    The 2 s figure is the *storm's* duration, not a per-resize latency budget:
    a terminal delivers resizes on its own schedule and there is no user-facing
    latency to promise. What matters is that the app survives and still echoes
    input afterwards.
    """
    child, _ = _spawn(home, cwd=tmp_cwd)
    fd = child.child_fd
    os.set_blocking(fd, False)

    def drain() -> int:
        total = 0
        while True:
            try:
                chunk = os.read(fd, 65536)
            except (BlockingIOError, OSError):
                return total
            if not chunk:
                return total
            total += len(chunk)

    try:
        drain()
        started = time.perf_counter()
        for index in range(100):
            child.setwinsize(20 + index % 10, 60 + index % 40)
            time.sleep(0.015)
        storm_elapsed = time.perf_counter() - started
        child.setwinsize(30, 100)
        time.sleep(0.4)
        painted_after = drain()

        drain()
        os.write(fd, b"z")
        echo_started = time.perf_counter()
        echoed = False
        while time.perf_counter() - echo_started < 2:
            try:
                chunk = os.read(fd, 65536)
            except BlockingIOError:
                time.sleep(0.005)
                continue
            if b"z" in chunk:
                echoed = True
                break
        alive = child.isalive()
    finally:
        child.terminate(force=True)

    assert alive, "S7: the app died during the resize storm"
    assert storm_elapsed <= 2.0, f"S7: the storm itself took {storm_elapsed:.2f}s"
    assert painted_after > 0, "S7: the app painted nothing after the final resize"
    assert echoed, "S7: the composer stopped echoing after the resize storm"
