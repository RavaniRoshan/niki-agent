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

import os
from pathlib import Path
from time import monotonic
from typing import ClassVar

from textual.binding import Binding, BindingType
from textual.theme import Theme

import deepagents_code
from deepagents_code.app import DeepAgentsApp
from deepagents_code.command_registry import COMMANDS, SlashCommand, get_slash_commands
from deepagents_code.domain.events import RunEvent, RunEventKind
from deepagents_code.niki import theme as niki_theme
from deepagents_code.niki.command_palette import CommandPaletteScreen
from deepagents_code.niki.message_store import NikiMessageStore
from deepagents_code.niki.turn_summary import TurnSummary, TurnSummaryData
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

    BINDINGS: ClassVar[list[BindingType]] = [
        *DeepAgentsApp.BINDINGS,
        Binding(
            "ctrl+shift+p",
            "open_command_palette",
            "Command Palette",
            show=False,
            priority=True,
        ),
    ]

    _turn_summary_events: dict[tuple[str, str], list[RunEvent]]
    _turn_summary_started_at: dict[tuple[str, str], float]

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

        Every Niki palette is registered so a light-terminal owner has
        somewhere to switch to; Niki defaults to the dark one. Set
        `NIKI_COLOR_DEPTH=16` to register the same variants with native ANSI
        colors for limited-color terminals.
        """
        super()._register_custom_themes()
        ansi16 = os.environ.get("NIKI_COLOR_DEPTH", "").strip() == "16"
        for name, palette in niki_theme.NIKI_PALETTES.items():
            selected_palette = (
                niki_theme.NIKI_ANSI16_PALETTES[name] if ansi16 else palette
            )
            self.register_theme(
                self._build_niki_theme(
                    name,
                    selected_palette,
                    dark=name != "niki-light",
                    ansi=ansi16,
                )
            )

    @staticmethod
    def _build_niki_theme(
        name: str, palette: dict[str, str], dark: bool, *, ansi: bool = False
    ) -> Theme:
        """Build a Textual `Theme` from a Niki token palette.

        Every token is also emitted as a CSS variable so `$niki-*` resolves in
        Niki's own `.tcss` overrides. Upstream's `get_css_variable_defaults`
        covers seven tokens with no Textual equivalent; registering all of them
        keeps a single naming scheme across both palettes.

        Args:
            name: Registry name for the theme.
            palette: Token name to RGB hex or a Textual ANSI color value.
            dark: Whether Textual should treat this as a dark theme.
            ansi: Whether palette colors are native ANSI indices rather than RGB.

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
            ansi=ansi,
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
        self._turn_summary_events = {}
        self._turn_summary_started_at = {}
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

    def action_open_command_palette(self) -> None:
        """Open a searchable picker over commands enabled by the registry."""
        enabled = {entry.name for entry in get_slash_commands()}
        commands = [command for command in COMMANDS if command.name in enabled]
        self.push_screen(
            CommandPaletteScreen(commands), self._dispatch_palette_selection
        )

    async def _dispatch_palette_selection(self, command: SlashCommand | None) -> None:
        """Return palette selections to the existing slash-command handler."""
        if command is not None:
            await self._handle_command(command.name)

    def _on_runtime_event(self, event: RunEvent) -> None:
        """Keep a turn's minimal tool evidence and render its terminal summary."""
        super()._on_runtime_event(event)
        key = (event.session_id, event.turn_id)

        if event.kind == RunEventKind.turn_started:
            self._turn_summary_events.setdefault(key, [])
            self._turn_summary_started_at.setdefault(key, monotonic())
            return

        if event.kind in {RunEventKind.tool_started, RunEventKind.tool_finished}:
            self._turn_summary_events.setdefault(key, []).append(event)
            return

        if event.kind not in {
            RunEventKind.turn_finished,
            RunEventKind.turn_failed,
            RunEventKind.turn_cancelled,
        }:
            return

        events = self._turn_summary_events.pop(key, [])
        events.append(event)
        started_at = self._turn_summary_started_at.pop(key, monotonic())
        summary = TurnSummary.from_events(events, max(0.0, monotonic() - started_at))
        self.call_later(
            lambda: self.run_worker(
                self._render_turn_summary(summary),
                group="niki-turn-summary",
                exit_on_error=False,
            )
        )

    async def _render_turn_summary(self, summary: TurnSummaryData) -> None:
        """Mount the event-derived result line in the transcript."""
        from deepagents_code.tui.widgets.messages import AppMessage

        await self._mount_message(AppMessage(TurnSummary.render(summary)))


__all__ = ["NikiApp"]
