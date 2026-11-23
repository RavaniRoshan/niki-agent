"""Tool registry port."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol, Sequence

from deepagents_code.application.ports.workspace import CommandResult
from deepagents_code.application.ports.permissions import ToolAction
from deepagents_code.domain.events import JsonObject


@dataclass(frozen=True, slots=True)
class ToolDescriptor:
    name: str
    description: str = ""


@dataclass(frozen=True, slots=True)
class ToolResult:
    output: str
    error: str | None = None


class ToolRegistryPort(Protocol):
    async def invoke(self, action: ToolAction) -> ToolResult:
        ...

    async def list_tools(self, workspace_id: str) -> Sequence[ToolDescriptor]:
        ...
