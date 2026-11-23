"""Command palette screen for Niki."""

from __future__ import annotations

from typing import Sequence

from deepagents_code.command_registry import SlashCommand


class CommandPaletteScreen:
    def __init__(self, commands: Sequence[SlashCommand]) -> None:
        self.commands = list(commands)

    def filter(self, query: str) -> list[SlashCommand]:
        q = query.lower()
        return [c for c in self.commands if q in c.name.lower() or q in (c.description or "").lower()]

    def invoke(self, command: SlashCommand, handler) -> None:
        handler(command)
