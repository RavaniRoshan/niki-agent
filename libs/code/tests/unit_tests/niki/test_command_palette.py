"""Tests for Niki's keyboard-driven command palette."""

from __future__ import annotations

from types import SimpleNamespace
from typing import ClassVar
from unittest.mock import AsyncMock

from deepagents_code.command_registry import BypassTier, SlashCommand
from deepagents_code.niki.app import NikiApp
from deepagents_code.niki.command_palette import CommandPaletteScreen


async def test_command_palette_filters_and_invokes_registered_command():
    """The modal filters, navigates, dismisses, and uses Niki's dispatcher."""
    from textual.app import App, ComposeResult
    from textual.binding import Binding
    from textual.widgets import Input, OptionList

    captured: list[tuple[CommandPaletteScreen, object]] = []
    dispatcher = SimpleNamespace(_handle_command=AsyncMock())
    dispatcher._dispatch_palette_selection = lambda command: (
        NikiApp._dispatch_palette_selection(dispatcher, command)
    )
    dispatcher.push_screen = lambda screen, callback: captured.append(
        (screen, callback)
    )
    NikiApp.action_open_command_palette(dispatcher)
    screen, callback = captured[0]

    class PaletteHost(App[None]):
        BINDINGS: ClassVar[list[Binding]] = [
            Binding("ctrl+shift+p", "open_palette", "Command Palette")
        ]

        def compose(self) -> ComposeResult:
            yield Input(id="chat-input")

        def on_mount(self) -> None:
            self.query_one("#chat-input", Input).focus()

        def action_open_palette(self) -> None:
            self.push_screen(screen, self.dispatch)

        async def dispatch(self, command: SlashCommand | None) -> None:
            await callback(command)  # type: ignore[operator]

    app = PaletteHost()
    palette_opened = False
    filtered_count = 0
    restored_focus_id = None
    second_selected = ""
    second_highlight = -1
    escape_closed = False
    escape_focus_id = None
    async with app.run_test(size=(80, 24)) as pilot:
        await pilot.pause()
        await pilot.press("ctrl+shift+p")
        await pilot.pause()
        palette_opened = isinstance(app.screen, CommandPaletteScreen)
        if palette_opened:
            await pilot.press("h", "e", "l", "p")
            await pilot.pause()
            options = app.screen.query_one(OptionList)
            filtered_count = options.option_count
            await pilot.press("enter")
            await pilot.pause()
            restored_focus_id = getattr(app.focused, "id", None)

            await pilot.press("ctrl+shift+p")
            await pilot.pause()
            if isinstance(app.screen, CommandPaletteScreen):
                second_selected = app.screen.filtered_commands[1].name
                await pilot.press("down")
                await pilot.pause()
                second_highlight = app.screen.query_one(OptionList).highlighted
                await pilot.press("enter")
                await pilot.pause()

            await pilot.press("ctrl+shift+p")
            await pilot.pause()
            await pilot.press("escape")
            await pilot.pause()
            escape_closed = not isinstance(app.screen, CommandPaletteScreen)
            escape_focus_id = getattr(app.focused, "id", None)

    assert palette_opened
    assert filtered_count == 1
    assert restored_focus_id == "chat-input"
    assert second_highlight == 1
    assert escape_closed
    assert escape_focus_id == "chat-input"
    assert len(dispatcher._handle_command.await_args_list) == 2
    assert dispatcher._handle_command.await_args_list[0].args == ("/help",)
    assert dispatcher._handle_command.await_args_list[1].args == (second_selected,)


def test_command_palette_search_includes_registry_aliases_and_keywords():
    commands = [
        SlashCommand(
            name="/help",
            description="Show help",
            bypass_tier=BypassTier.QUEUED,
            hidden_keywords="assistance guide",
            aliases=("/h",),
        ),
        SlashCommand(
            name="/status",
            description="Show current status",
            bypass_tier=BypassTier.QUEUED,
        ),
    ]
    screen = CommandPaletteScreen(commands)

    assert [command.name for command in screen.filter("GUIDE")] == ["/help"]
    assert [command.name for command in screen.filter("/h")] == ["/help"]
