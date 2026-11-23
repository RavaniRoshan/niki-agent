"""Workspace adapter wrapping current backend facilities."""

from __future__ import annotations

import os
from typing import Any, Sequence

from deepagents_code.application.ports.permissions import (
    ApprovalDecision,
    ToolAction,
)
from deepagents_code.application.ports.workspace import CommandResult, WorkspacePort
from deepagents_code.approval_mode import ApprovalMode
from deepagents_code.domain.events import JsonObject

_MUTATING_PREFIXES = ("write", "edit", "run", "execute", "delete", "move", "create", "patch")
_MUTATING_NAMES = {"write_file", "edit_file", "run_command", "bash", "shell"}


def _is_mutating(name: str) -> bool:
    low = name.lower()
    if low in _MUTATING_NAMES:
        return True
    return any(low.startswith(p) for p in _MUTATING_PREFIXES)


class DefaultApprovalPolicy:
    def decide(self, action: ToolAction, mode: ApprovalMode) -> ApprovalDecision:
        if mode in (ApprovalMode.AUTO, ApprovalMode.YOLO):
            return ApprovalDecision.allow
        if mode == ApprovalMode.MANUAL:
            if _is_mutating(action.name):
                return ApprovalDecision.request
            return ApprovalDecision.allow
        return ApprovalDecision.deny


class InMemoryWorkspaceAdapter:
    """In-memory WorkspacePort for tests and lightweight use."""

    def __init__(self, root: str = "/workspace") -> None:
        self.root = os.path.normpath(root)
        self._files: dict[str, str] = {}

    def _contain(self, path: str) -> str:
        if not isinstance(path, str) or not path:
            raise ValueError("path must be non-empty str")
        # Resolve symlinks/realpath if the path exists on disk; otherwise normpath.
        candidate = path
        if os.path.isabs(candidate):
            full = os.path.normpath(candidate)
        else:
            full = os.path.normpath(os.path.join(self.root, candidate))
        # symlink escape check against real filesystem
        try:
            real = os.path.realpath(full)
        except OSError:
            real = full
        root_real = os.path.realpath(self.root) if os.path.exists(self.root) else self.root
        if real != root_real and not real.startswith(root_real + os.sep):
            raise ValueError("path escapes workspace")
        if full != self.root and not full.startswith(self.root + os.sep):
            raise ValueError("path escapes workspace")
        return full

    async def read_text(self, path: str) -> str:
        full = self._contain(path)
        if full not in self._files:
            raise FileNotFoundError(path)
        return self._files[full]

    async def write_text(self, path: str, content: str) -> None:
        full = self._contain(path)
        self._files[full] = content

    async def run_command(
        self, argv: Sequence[str], *, cwd: str, timeout: float
    ) -> CommandResult:
        self._contain(cwd) if cwd else None
        return CommandResult(returncode=0, stdout="", stderr="")


class DeepAgentsWorkspaceAdapter:
    """Thin adapter wrapping a backend (BackendProtocol-like)."""

    def __init__(self, backend: Any, workspace_id: str) -> None:
        self.backend = backend
        self.workspace_id = workspace_id

    async def read_text(self, path: str) -> str:
        if hasattr(self.backend, "read"):
            return await _maybe_await(self.backend.read, path)
        raise NotImplementedError

    async def write_text(self, path: str, content: str) -> None:
        if hasattr(self.backend, "write"):
            await _maybe_await(self.backend.write, path, content)
            return
        raise NotImplementedError

    async def run_command(
        self, argv: Sequence[str], *, cwd: str, timeout: float
    ) -> CommandResult:
        if hasattr(self.backend, "execute"):
            res = await _maybe_await(self.backend.execute, list(argv), cwd=cwd, timeout=timeout)
            if isinstance(res, CommandResult):
                return res
            return CommandResult(
                returncode=getattr(res, "returncode", 0),
                stdout=getattr(res, "stdout", str(res)),
                stderr=getattr(res, "stderr", ""),
            )
        raise NotImplementedError


async def _maybe_await(fn, *a, **kw):
    import asyncio
    import inspect

    result = fn(*a, **kw)
    if inspect.isawaitable(result):
        return await result
    return result
