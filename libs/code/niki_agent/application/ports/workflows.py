"""Workflow repository port."""

from __future__ import annotations

from typing import TYPE_CHECKING, Protocol

if TYPE_CHECKING:
    from niki_agent.domain.workflow import WorkflowDefinition, WorkflowStep


class WorkflowRepository(Protocol):
    """Persist reusable workflow definitions independently of run state."""

    def load(self, name: str) -> WorkflowDefinition | None:
        """Load one definition, returning `None` when it is not saved."""
        ...

    def save(self, definition: WorkflowDefinition) -> None:
        """Atomically save or replace a definition by its name."""
        ...

    def list(self) -> tuple[WorkflowDefinition, ...]:
        """Return definitions in stable name order."""
        ...


class WorkflowStepPort(Protocol):
    """Execute a stage through the existing agent/runtime capability."""

    async def run_step(
        self, step: WorkflowStep, *, session_id: str, workflow_name: str
    ) -> str:
        """Run one stage and return its user-visible result summary."""
        ...
