"""Framework-free workflow types."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class WorkflowStep:
    step_id: str
    instruction: str
    depends_on: tuple[str, ...] = ()


@dataclass(frozen=True, slots=True)
class WorkflowDefinition:
    name: str
    version: int
    steps: tuple[WorkflowStep, ...]

    def __post_init__(self):
        ids = [s.step_id for s in self.steps]
        if len(set(ids)) != len(ids):
            raise ValueError("duplicate step ids")
        id_set = set(ids)
        for s in self.steps:
            for d in s.depends_on:
                if d not in id_set:
                    raise ValueError(f"unknown dependency {d}")
        # cycle check
        adj = {s.step_id: list(s.depends_on) for s in self.steps}
        WHITE, GRAY, BLACK = 0, 1, 2
        color = {sid: WHITE for sid in id_set}

        def dfs(node):
            color[node] = GRAY
            for dep in adj.get(node, []):
                if color[dep] == GRAY:
                    raise ValueError("cycle detected")
                if color[dep] == WHITE:
                    dfs(dep)
            color[node] = BLACK

        for sid in id_set:
            if color[sid] == WHITE:
                dfs(sid)


@dataclass(frozen=True, slots=True)
class WorkflowRun:
    workflow_name: str
    session_id: str
    completed_steps: tuple[str, ...] = ()
