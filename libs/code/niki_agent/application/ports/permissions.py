"""Permission ports."""

from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum
from typing import TYPE_CHECKING, Protocol, runtime_checkable

if TYPE_CHECKING:
    from niki_agent.approval_mode import ApprovalMode
    from niki_agent.domain.events import JsonObject


class ApprovalDecision(StrEnum):
    """How a tool action may proceed."""

    allow = "allow"
    deny = "deny"
    request = "request"


@dataclass(frozen=True, slots=True)
class ToolAction:
    """A tool invocation awaiting a decision."""

    name: str
    arguments: JsonObject
    workspace_id: str


@runtime_checkable
class ApprovalPolicy(Protocol):
    """Policy boundary for tool approval decisions."""

    def decide(self, action: ToolAction, mode: ApprovalMode) -> ApprovalDecision:
        """Decide whether an action is allowed, denied, or escalated."""
        ...
