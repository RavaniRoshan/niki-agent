"""M4: selection, copy, OSC 52, and releasing the mouse.

The half of M4 that can be decided here is the copy path. Upstream ships
`clipboard.py` with an OSC 52 writer and a tmux passthrough variant, plus
Textual's `copy_to_clipboard`. What is asserted is that the sequence is
well-formed and correctly wrapped for tmux -- because a malformed OSC 52 is
worse than none: a clipboard that silently contains the wrong bytes, or a
terminal left in a DCS state.

What cannot be decided here is the part a human must check: whether OSC 52
actually reaches the user's clipboard across a real ssh hop, whether Textual's
selection works when the mouse is captured, and whether a mouse-release toggle
exists. Those are in `docs/niki/OWNER_VERIFY.md`.
"""

from __future__ import annotations

import base64
from typing import TYPE_CHECKING, Any

import pytest

if TYPE_CHECKING:
    from typing import Self


def _capture(monkeypatch: pytest.MonkeyPatch) -> list[str]:
    """Capture what the OSC 52 writer emits, without a real terminal.

    `_copy_osc52` opens `/dev/tty` directly and exposes no seam, so the write is
    intercepted at `pathlib.Path.open`. That is deliberately the narrowest hook
    available: the alternative -- skipping these tests because there is no tty in
    CI -- would leave the clipboard sequence entirely unverified, which is the
    one place a malformed write does real damage.
    """
    import pathlib

    written: list[str] = []

    class _Tty:
        # `_copy_osc52` uses `with ... .open(...)`, so the stand-in must be a
        # context manager, not just a writer.
        encoding = "utf-8"

        def __enter__(self) -> Self:
            return self

        def __exit__(self, *exc: object) -> None:
            return None

        def write(self, data: str) -> int:
            written.append(data)
            return len(data)

        def flush(self) -> None:
            return None

    real_open: Any = pathlib.Path.open

    def _fake_open(self: pathlib.Path, *args: Any, **kwargs: Any) -> Any:  # noqa: ANN401
        if str(self) == "/dev/tty":
            return _Tty()
        return real_open(self, *args, **kwargs)  # type: ignore[arg-type]

    monkeypatch.setattr(pathlib.Path, "open", _fake_open)  # ty: ignore[invalid-assignment]
    return written


def test_m4_osc52_sequence_is_well_formed(monkeypatch: pytest.MonkeyPatch) -> None:
    """M4: the clipboard write must be a valid OSC 52 sequence."""
    from deepagents_code.clipboard import _copy_osc52

    written = _capture(monkeypatch)
    _copy_osc52("hello niki")
    seq = written[-1]

    assert seq.startswith("\x1b]52;c;"), f"not an OSC 52 clipboard write: {seq!r}"
    assert seq.endswith("\x07"), f"OSC 52 is not BEL-terminated: {seq!r}"
    payload = seq[len("\x1b]52;c;") : -1]
    assert base64.b64decode(payload).decode() == "hello niki", (
        "the OSC 52 payload does not decode back to the text"
    )


def test_m4_osc52_is_wrapped_for_tmux(monkeypatch: pytest.MonkeyPatch) -> None:
    """M4: inside tmux the sequence must be DCS-wrapped or it is swallowed."""
    import os

    from deepagents_code.clipboard import _copy_osc52

    written = _capture(monkeypatch)
    previous = os.environ.get("TMUX")
    monkeypatch.setenv("TMUX", "/tmp/tmux-1000/default,123,0")
    try:
        _copy_osc52("wrapped")
    finally:
        if previous is None:
            os.environ.pop("TMUX", None)
        else:
            os.environ["TMUX"] = previous

    seq = written[-1]

    assert seq.startswith("\x1bPtmux;"), f"not tmux-wrapped inside TMUX: {seq!r}"
    assert "\x1b\\" in seq, "the tmux DCS passthrough is not terminated"


def test_m4_the_keymap_advertises_a_copy_action() -> None:
    """M4: copy must be reachable by key, not only by an undiscoverable menu."""
    from textual.screen import Screen

    assert hasattr(Screen, "action_copy_text"), (
        "M4: no copy action is bound on Screen; the keyboard equivalent of "
        "select-and-copy is missing"
    )


def test_m4_mouse_release_toggle_is_owner_verify() -> None:
    """M4: no mouse-capture toggle exists, so this half stays OWNER-VERIFY.

    Characterised rather than skipped: when a toggle is added, this test fails
    and forces the row to be updated in the same change.
    """
    from deepagents_code.app import DeepAgentsApp

    toggles = [
        name
        for name in dir(DeepAgentsApp)
        if "mouse" in name.lower() and "toggle" in name.lower()
    ]
    assert not toggles, (
        f"M4: a mouse toggle now exists ({toggles}). Wire it to a key, add the "
        "OWNER-VERIFY step, and mark M4 complete in docs/niki/CHECKLIST.md."
    )


@pytest.mark.parametrize(
    ("text", "why"),
    [
        ("", "empty selection"),
        ("line\nline", "multi-line selection"),
        ("emoji and 中文", "non-ASCII selection"),
        ("tab\tand  spaces", "whitespace-sensitive selection"),
    ],
)
def test_m4_osc52_handles_awkward_payloads(
    text: str, why: str, monkeypatch: pytest.MonkeyPatch
) -> None:
    """M4: whatever is selected must survive the base64 round trip."""
    from deepagents_code.clipboard import _copy_osc52

    written = _capture(monkeypatch)
    _copy_osc52(text)
    seq = written[-1]
    payload = seq[len("\x1b]52;c;") : -1]

    assert base64.b64decode(payload).decode("utf-8") == text, (
        f"OSC 52 mangled the payload ({why})"
    )
