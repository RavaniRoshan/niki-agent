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

from textual.theme import Theme

import deepagents_code
from deepagents_code.app import DeepAgentsApp
from deepagents_code.niki import theme as niki_theme
from deepagents_code.niki.message_store import NikiMessageStore
from deepagents_code.theme import DEFAULT_THEME as _UPSTREAM_DEFAULT_THEME
from deepagents_code.tui.widgets.chat_input import ChatTextArea
from deepagents_code.tui.widgets.message_store import MessageStore


class NikiApp(DeepAgentsApp):
    """`DeepAgentsApp` with Niki's presentation defaults applied."""

    TITLE = "Niki Agent"
    """Window and header title. Upstream sets `TITLE = "Deep Agents"`
    (`app.py:3331`); it is a plain class attribute, so rebranding needs no
    string surgery anywhere else."""

    #: Blink is opt-in; see the module docstring for the measurement. Assign to
    #: the instance before mounting to override.
    blink_cursor: bool = False

    #: The theme Niki starts in. Upstream defaults to `"langchain"`
    #: (`theme.DEFAULT_THEME`); an owner can still switch with `/theme`.
    DEFAULT_THEME_NAME: ClassVar[str] = niki_theme.NIKI_THEME_NAME

    #: Textual resolves `CSS_PATH` relative to the *subclass's* module, so
    #: inheriting upstream's bare `"app.tcss"` would look for a stylesheet in
    #: `deepagents_code/niki/`. Point at the real file explicitly instead.
    #: Niki's own sheet is listed second so it wins the cascade: it overrides
    #: upstream's rules without editing or deleting any of them.
    CSS_PATH: ClassVar[list[str]] = [
        str(Path(deepagents_code.__file__).parent / "app.tcss"),
        str(Path(__file__).parent / "niki.tcss"),
    ]

    def _register_custom_themes(self) -> None:
        """Register upstream's themes, then add Niki's through the same path.

        Upstream builds `Theme` objects from `theme.get_registry()` and hands them
        to Textual's `register_theme` (app.py:24963). Niki registers through that
        same mechanism rather than around it, so `/theme` lists the Niki theme
        beside the built-ins and any owner CSS override keeps working.

        Both palettes are registered so a light-terminal owner has somewhere to
        switch to; Niki defaults to the dark one.
        """
        super()._register_custom_themes()
        for name, palette in niki_theme.NIKI_PALETTES.items():
            self.register_theme(
                self._build_niki_theme(name, palette, dark=name != "niki-light")
            )

    @staticmethod
    def _build_niki_theme(name: str, palette: dict[str, str], dark: bool) -> Theme:
        """Build a Textual `Theme` from a Niki token palette.

        Every token is also emitted as a CSS variable so `$niki-*` resolves in
        Niki's own `.tcss` overrides. Upstream's `get_css_variable_defaults`
        covers seven tokens with no Textual equivalent; registering all of them
        keeps a single naming scheme across both palettes.

        Args:
            name: Registry name for the theme.
            palette: Token name to `#RRGGBB` value.
            dark: Whether Textual should treat this as a dark theme.

        Returns:
            A `Theme` ready for Textual's `register_theme`.
        """
        return Theme(
            name=name,
            primary=palette["primary"],
            secondary=palette["secondary"],
            accent=palette["accent"],
            foreground=palette["foreground"],
            background=palette["background"],
            surface=palette["surface"],
            panel=palette["panel"],
            warning=palette["warning"],
            error=palette["error"],
            success=palette["success"],
            dark=dark,
            variables={
                **{
                    f"niki-{token.replace('_', '-')}": value
                    for token, value in palette.items()
                },
                "footer-key-foreground": palette["primary"],
            },
        )

    async def on_mount(self) -> None:
        """Mount upstream's app, then schedule the Niki presentation defaults."""
        await super().on_mount()
        self._prefer_niki_theme()
        self._install_niki_message_store()
        # The composer is built after mount, so `on_mount` alone finds nothing.
        # `call_after_refresh` runs once the first frame is up, by which point
        # the chat input exists.
        self.call_after_refresh(self._apply_cursor_blink)

    def _prefer_niki_theme(self) -> None:
        """Switch to the Niki theme unless the owner explicitly chose another.

        Upstream resolves the theme through a module-level function
        (`_load_theme_preference`, app.py:1487) whose final fallback is
        `theme.DEFAULT_THEME`. Mutating that module attribute was the obvious way
        to change Niki's default and it leaked: a `NikiApp` built earlier in the
        process left `DEFAULT_THEME` pointing at `"niki"`, so a later plain
        `DeepAgentsApp` asked for a theme it had never registered and failed.
        Real apps run one per process and never noticed; the test suite did.

        So instead: if the app landed on the upstream default, nothing chose it,
        and Niki's default applies. Managed policy, `DEEPAGENTS_CODE_THEME`, and
        a saved user preference all still win, because each of those resolves to
        something other than the default.
        """
        if (
            self.theme == _UPSTREAM_DEFAULT_THEME
            and self.DEFAULT_THEME_NAME in self.available_themes
        ):
            self.theme = self.DEFAULT_THEME_NAME

    def _install_niki_message_store(self) -> None:
        """Swap in the bounded-window store.

        `DeepAgentsApp.__init__` assigns `self._message_store = MessageStore()`
        (app.py:4535). Overwriting the attribute afterwards is the extension
        route -- upstream's constructor takes no store argument, and adding one
        would edit the 31,916-line module for a single line of effect.
        """
        if not isinstance(self._message_store, MessageStore):
            return
        self._message_store = NikiMessageStore()

    def _apply_cursor_blink(self) -> None:
        """Push the blink preference onto every composer text area."""
        for text_area in self.query(ChatTextArea):
            text_area.cursor_blink = self.blink_cursor


__all__ = ["NikiApp"]
