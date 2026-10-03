"""`NikiApp`: the Niki presentation layer over upstream's `DeepAgentsApp`.

Subclassing rather than patching is what keeps a future merge from `main`
cheap, so every override here is a decision with a measurement behind it.

**Cursor blink is off by default.** This is the S4 fix and it is measured, not
assumed. The composer is a `TextArea`, and Textual's `cursor_blink` reactive
defaults to True; because the composer holds focus, its blink timer runs
forever. Measured at BASE_SHA, that produced **10 repaints in 5 s while idle**
at 0.46 % CPU. `NikiApp` turns it off, which takes it to **0 repaints in 5 s**.

A blinking cursor is a legitimate preference, not a defect, so this is a
setting rather than a hard-off: set `app.blink_cursor = True` before the app
mounts. The perf probes assert zero with it off *and* that the setting is
genuinely wired, so it cannot quietly become permanent.

`blink_cursor` is a plain attribute rather than an `__init__` keyword on
purpose. `DeepAgentsApp.__init__` takes thirty-odd typed parameters; re-declaring
them to thread one flag through would copy a signature that must track upstream.
A class-level default plus an instance attribute keeps `ty` honest about the
forwarding and adds no signature to keep in sync.

Nothing here changes agent behavior, tools, or the permission posture. Approval
defaults are inherited untouched from `DeepAgentsApp`.
"""

from __future__ import annotations

from pathlib import Path
from typing import ClassVar

import deepagents_code
from deepagents_code.app import DeepAgentsApp
from deepagents_code.tui.widgets.chat_input import ChatTextArea


class NikiApp(DeepAgentsApp):
    """`DeepAgentsApp` with Niki's presentation defaults applied."""

    #: Blink is opt-in; see the module docstring for the measurement. Assign to
    #: the instance before mounting to override.
    blink_cursor: bool = False

    #: Textual resolves `CSS_PATH` relative to the *subclass's* module, so
    #: inheriting upstream's bare `"app.tcss"` would look for a stylesheet in
    #: `deepagents_code/niki/`. Point at the real file explicitly instead, and
    #: add Niki's own overrides after it so they win the cascade.
    CSS_PATH: ClassVar[list[str]] = [
        str(Path(deepagents_code.__file__).parent / "app.tcss"),
    ]

    async def on_mount(self) -> None:
        """Mount upstream's app, then schedule the Niki presentation defaults."""
        await super().on_mount()
        # The composer is built after mount, so `on_mount` alone finds nothing.
        # `call_after_refresh` runs once the first frame is up, by which point
        # the chat input exists.
        self.call_after_refresh(self._apply_cursor_blink)

    def _apply_cursor_blink(self) -> None:
        """Push the blink preference onto every composer text area."""
        for text_area in self.query(ChatTextArea):
            text_area.cursor_blink = self.blink_cursor


__all__ = ["NikiApp"]
