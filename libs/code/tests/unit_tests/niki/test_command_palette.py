"""Tests for command palette."""

from __future__ import annotations

from deepagents_code.command_registry import SlashCommand
from deepagents_code.niki.command_palette import CommandPaletteScreen


def test_command_palette_filters_and_invokes_registered_command():
    cmds = [
        SlashCommand(name="help", description="show help", bypass_tier=frozenset()),
        SlashCommand(name="status", description="show status", bypass_tier=frozenset()),
    ]
    screen = CommandPaletteScreen(cmds)
    filtered = screen.filter("hel")
    assert len(filtered) == 1
    assert filtered[0].name == "help"
    # case-insensitive
    assert len(screen.filter("HEL")) == 1
    invoked = {"n": 0}

    def handler(c):
        invoked["n"] += 1

    screen.invoke(filtered[0], handler)
    assert invoked["n"] == 1
