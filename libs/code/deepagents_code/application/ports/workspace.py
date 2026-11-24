"""Workspace port."""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Protocol, runtime_checkable

if TYPE_CHECKING:
    from collections.abc import Sequence


@dataclass(frozen=True, slots=True)
class CommandResult:
    """Command status and backend output; `-1` means status is unknown."""

    returncode: int
    stdout: str = ""
    stderr: str = ""


@runtime_checkable
class WorkspacePort(Protocol):
    """Async workspace operations scoped to one validated identity."""

    async def read_text(self, path: str) -> str:
        """Return UTF-8 text at `path`."""
        ...

    async def write_text(self, path: str, content: str) -> None:
        """Write UTF-8 `content` at `path`."""
        ...

    async def run_command(
        self,
        argv: Sequence[str],
        *,
        cwd: str,
        timeout: float,  # noqa: ASYNC109
    ) -> CommandResult:
        """Run an argv command from `cwd` with a timeout in seconds."""
        ...
