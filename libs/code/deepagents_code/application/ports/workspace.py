"""Workspace port."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol, Sequence, runtime_checkable


@dataclass(frozen=True, slots=True)
class CommandResult:
    returncode: int
    stdout: str = ""
    stderr: str = ""


@runtime_checkable
class WorkspacePort(Protocol):
    async def read_text(self, path: str) -> str:
        ...

    async def write_text(self, path: str, content: str) -> None:
        ...

    async def run_command(
        self, argv: Sequence[str], *, cwd: str, timeout: float
    ) -> CommandResult:
        ...
