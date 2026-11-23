"""Permission ports."""

from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum
from typing import Protocol, runtime_checkable

from deepagents_code.approval_mode import ApprovalMode
from deepagents_code.domain.events import JsonObject


class ApprovalDecision(StrEnum):
    allow = "allow"
    deny = "deny"
    request = "request"


@dataclass(frozen=True, slots=True)
class ToolAction:
    name: str
    arguments: JsonObject
    workspace_id: str


@runtime_checkable
class ApprovalPolicy(Protocol):
    def decide(self, action: ToolAction, mode: ApprovalMode) -> ApprovalDecision:
        ...
