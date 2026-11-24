"""Approval-policy adapter for the coding agent's existing HITL path."""

from __future__ import annotations

from typing import TYPE_CHECKING

from deepagents_code.application.ports.permissions import (
    ApprovalDecision,
    ToolAction,
)
from deepagents_code.approval_mode import ApprovalMode

if TYPE_CHECKING:
    from collections.abc import Collection


class DefaultApprovalPolicy:
    """Translate the live HITL mode for an action already selected for gating.

    `interrupt_on` remains the source of truth for which actions are gated.
    This policy only maps that existing gate and the server-resolved mode to
    the application port's decision vocabulary.
    """

    def __init__(
        self,
        *,
        workspace_id: str | None = None,
        gated_tool_names: Collection[str] | None = None,
        auto_mode_enabled: bool = True,
    ) -> None:
        """Bind decisions to the active workspace and existing gated tools.

        Args:
            workspace_id: Validated identity of the active workspace, if bound.
            gated_tool_names: Existing `interrupt_on` names. `None` means the
                caller has already selected a gated action.
            auto_mode_enabled: Whether this graph's current HITL path permits
                Auto mode to bypass a manual interrupt.
        """
        self.workspace_id = workspace_id
        self.gated_tool_names = (
            frozenset(gated_tool_names) if gated_tool_names is not None else None
        )
        self.auto_mode_enabled = auto_mode_enabled

    def decide(self, action: ToolAction, mode: ApprovalMode) -> ApprovalDecision:
        """Return the HITL decision without creating another mode store.

        Returns:
            The port decision for the selected action and current mode.
        """
        if self.workspace_id is not None and action.workspace_id != self.workspace_id:
            return ApprovalDecision.deny
        if (
            self.gated_tool_names is not None
            and action.name not in self.gated_tool_names
        ):
            return ApprovalDecision.allow
        if mode is ApprovalMode.YOLO:
            return ApprovalDecision.allow
        if mode is ApprovalMode.AUTO and self.auto_mode_enabled:
            return ApprovalDecision.allow
        if mode in {ApprovalMode.AUTO, ApprovalMode.MANUAL}:
            return ApprovalDecision.request
        return ApprovalDecision.deny
