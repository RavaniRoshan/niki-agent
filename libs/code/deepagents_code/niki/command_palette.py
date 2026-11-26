"""Keyboard-driven slash-command palette for Niki."""

from __future__ import annotations

from typing import TYPE_CHECKING, ClassVar, override

from textual.binding import Binding
from textual.containers import Vertical
from textual.screen import ModalScreen
from textual.widgets import Input, Label, OptionList
from textual.widgets.option_list import Option

from deepagents_code.command_registry import SlashCommand

if TYPE_CHECKING:
    from collections.abc import Sequence

    from textual.app import ComposeResult


class CommandPaletteScreen(ModalScreen[SlashCommand | None]):
    """Filter registered slash commands and return the selected definition."""

    BINDINGS: ClassVar[list[Binding]] = [
        Binding("escape", "cancel", "Close palette", show=False, priority=True),
        Binding("up", "cursor_up", "Previous command", show=False, priority=True),
        Binding("down", "cursor_down", "Next command", show=False, priority=True),
        Binding("enter", "choose_selected", "Run command", show=False, priority=True),
    ]

    def __init__(self, commands: Sequence[SlashCommand]) -> None:
        """Initialize the screen with the active command definitions."""
        super().__init__()
        self.commands = tuple(commands)
        self.filtered_commands: list[SlashCommand] = []

    @override
    def compose(self) -> ComposeResult:
        """Build the search field and keyboard-selectable result list.

        Yields:
            The command palette container and its child widgets.
        """
        with Vertical(id="command-palette"):
            yield Label("Command palette", id="command-palette-title")
            yield Input(placeholder="Search commands", id="command-palette-query")
            yield OptionList(id="command-palette-options")

    def on_mount(self) -> None:
        """Show all commands initially and focus the search field."""
        self._update_options("")
        self.query_one("#command-palette-query", Input).focus()

    def filter(self, query: str) -> list[SlashCommand]:
        """Return commands matching visible or registry-provided search text."""
        needle = query.strip().casefold()
        if not needle:
            return list(self.commands)
        return [
            command
            for command in self.commands
            if needle
            in " ".join(
                (
                    command.name,
                    command.description,
                    command.hidden_keywords,
                    *command.aliases,
                )
            ).casefold()
        ]

    def on_input_changed(self, event: Input.Changed) -> None:
        """Refresh results as the query changes."""
        if event.input.id == "command-palette-query":
            self._update_options(event.value)

    def on_input_submitted(self, event: Input.Submitted) -> None:
        """Treat Enter in the search field as command selection."""
        if event.input.id == "command-palette-query":
            event.stop()
            self.action_choose_selected()

    def on_option_list_option_selected(self, event: OptionList.OptionSelected) -> None:
        """Return a result selected with Enter or a mouse click."""
        event.stop()
        self._dismiss_index(event.option_index)

    def action_cancel(self) -> None:
        """Dismiss the palette without dispatching a command."""
        self.dismiss(None)

    def action_cursor_up(self) -> None:
        """Move the highlighted command up one row."""
        self.query_one("#command-palette-options", OptionList).action_cursor_up()

    def action_cursor_down(self) -> None:
        """Move the highlighted command down one row."""
        self.query_one("#command-palette-options", OptionList).action_cursor_down()

    def action_choose_selected(self) -> None:
        """Dismiss with the highlighted command, if a result exists."""
        options = self.query_one("#command-palette-options", OptionList)
        if options.highlighted is not None:
            self._dismiss_index(options.highlighted)

    def _update_options(self, query: str) -> None:
        options = self.query_one("#command-palette-options", OptionList)
        self.filtered_commands = self.filter(query)
        options.clear_options()
        for command in self.filtered_commands:
            label = command.name
            if command.argument_hint:
                label = f"{label} {command.argument_hint}"
            if command.description:
                label = f"{label}  {command.description}"
            options.add_option(Option(label))
        if self.filtered_commands:
            options.highlighted = 0

    def _dismiss_index(self, index: int) -> None:
        if 0 <= index < len(self.filtered_commands):
            self.dismiss(self.filtered_commands[index])
