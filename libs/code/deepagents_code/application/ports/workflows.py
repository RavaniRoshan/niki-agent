"""Workflow repository port."""

from __future__ import annotations

from typing import Protocol

from deepagents_code.domain.workflow import WorkflowDefinition


class WorkflowRepository(Protocol):
    def load(self, name: str) -> WorkflowDefinition | None:
        ...

    def save(self, definition: WorkflowDefinition) -> None:
        ...
