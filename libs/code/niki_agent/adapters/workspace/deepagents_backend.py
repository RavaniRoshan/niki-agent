"""Workspace adapters for in-memory use and Deep Agents backends."""

from __future__ import annotations

import math
import shlex
from collections.abc import Sequence
from pathlib import Path
from typing import TYPE_CHECKING, cast

if TYPE_CHECKING:
    from niki_agent.sdk.backends.protocol import BackendProtocol, SandboxBackendProtocol

from niki_agent.application.ports.workspace import CommandResult


def _resolve_workspace_path(root: Path, path: str) -> Path:
    """Resolve a path while keeping traversal and symlink escapes inside `root`.

    Args:
        root: Server-selected root directory for the workspace.
        path: Absolute path or a path relative to `root`.

    Returns:
        The resolved path, guaranteed to be inside `root`.

    Raises:
        ValueError: If the path is empty, invalid, or escapes the workspace.
    """
    if not isinstance(path, str) or not path:
        msg = "path must be non-empty str"
        raise ValueError(msg)
    candidate = Path(path)
    full = candidate if candidate.is_absolute() else root / candidate
    try:
        resolved_root = root.resolve(strict=False)
        resolved_full = full.resolve(strict=False)
    except (OSError, RuntimeError) as exc:
        msg = "path cannot be resolved within the workspace"
        raise ValueError(msg) from exc
    if resolved_full != resolved_root and resolved_root not in resolved_full.parents:
        msg = "path escapes workspace"
        raise ValueError(msg)
    return resolved_full


class InMemoryWorkspaceAdapter:
    """In-memory WorkspacePort used by tests and lightweight callers."""

    def __init__(self, root: str = "/workspace") -> None:
        """Create an isolated in-memory workspace.

        Args:
            root: Path boundary used for every operation.
        """
        self.root = Path(root).resolve(strict=False)
        self._files: dict[str, str] = {}

    def _contain(self, path: str) -> str:
        return str(_resolve_workspace_path(self.root, path))

    async def read_text(self, path: str) -> str:
        """Read a text file from this workspace.

        Returns:
            The stored text content.

        Raises:
            FileNotFoundError: If the file has not been written.
        """
        full = self._contain(path)
        if full not in self._files:
            raise FileNotFoundError(path)
        return self._files[full]

    async def write_text(self, path: str, content: str) -> None:
        """Write a text file after enforcing the workspace path boundary."""
        self._files[self._contain(path)] = content

    async def run_command(
        self,
        argv: Sequence[str],
        *,
        cwd: str,
        timeout: float,  # noqa: ASYNC109
    ) -> CommandResult:
        """Validate a command directory without launching a process.

        Returns:
            A successful empty result for this non-executing test adapter.
        """
        del argv, timeout
        _resolve_workspace_path(self.root, cwd)
        return CommandResult(returncode=0)


class DeepAgentsWorkspaceAdapter:
    """Adapt an already workspace-bound backend to the application port."""

    def __init__(
        self, backend: BackendProtocol, *, workspace_id: str, workspace_root: str
    ) -> None:
        """Wrap a backend already selected for one workspace.

        Args:
            backend: Deep Agents backend that owns file and command execution.
            workspace_id: Validated server identity for this workspace.
            workspace_root: Canonical root directory bound to the runtime.

        Raises:
            ValueError: If `workspace_id` or `workspace_root` is empty.
        """
        if not workspace_id or not workspace_root:
            msg = "workspace_id and workspace_root must be non-empty"
            raise ValueError(msg)
        self.backend = backend
        self.workspace_id = workspace_id
        self.workspace_root = Path(workspace_root).resolve(strict=False)
        self._workspace_identity_bound = workspace_id != "process-default"

    def bind_workspace(self, *, workspace_id: str, workspace_root: str) -> None:
        """Bind this adapter to the server-validated workspace identity.

        Raises:
            ValueError: If the identity or root is empty, or conflicts with an
                existing binding.
        """
        if not workspace_id or not workspace_root:
            msg = "workspace_id and workspace_root must be non-empty"
            raise ValueError(msg)
        resolved_root = Path(workspace_root).resolve(strict=False)
        if self._workspace_identity_bound and (
            workspace_id != self.workspace_id or resolved_root != self.workspace_root
        ):
            msg = "workspace adapter is already bound to a different identity"
            raise ValueError(msg)
        self.workspace_id = workspace_id
        self.workspace_root = resolved_root
        self._workspace_identity_bound = True

    def _path(self, path: str) -> str:
        return str(_resolve_workspace_path(self.workspace_root, path))

    async def read_text(self, path: str) -> str:
        """Read UTF-8 text through the backend.

        Returns:
            The file's text content.

        Raises:
            FileNotFoundError: If the backend returned no file content.
            OSError: If the backend reports a read error.
            UnicodeError: If the backend returned non-text content.
        """
        full_path = self._path(path)
        result = await self.backend.aread(full_path)
        if result.error is not None:
            raise OSError(result.error)
        if result.file_data is None:
            raise FileNotFoundError(full_path)
        if result.file_data.get("encoding") != "utf-8":
            msg = f"workspace file is not UTF-8 text: {full_path}"
            raise UnicodeError(msg)
        return result.file_data["content"]

    async def write_text(self, path: str, content: str) -> None:
        """Write text through the backend and surface backend failures.

        Raises:
            OSError: If the backend reports a write error.
        """
        result = await self.backend.awrite(self._path(path), content)
        if result.error is not None:
            raise OSError(result.error)

    async def run_command(
        self,
        argv: Sequence[str],
        *,
        cwd: str,
        timeout: float,  # noqa: ASYNC109
    ) -> CommandResult:
        """Run an argv command in a workspace directory.

        Returns:
            Combined backend output and its exit status.

        Raises:
            ValueError: If arguments, timeout, or `cwd` are invalid.
        """
        if (
            not isinstance(argv, Sequence)
            or isinstance(argv, (str, bytes))
            or not argv
            or any(not isinstance(part, str) or "\x00" in part for part in argv)
        ):
            msg = "argv must contain at least one NUL-free string"
            raise ValueError(msg)
        if (
            isinstance(timeout, bool)
            or not isinstance(timeout, (int, float))
            or not math.isfinite(timeout)
            or timeout < 0
        ):
            msg = "timeout must be a finite non-negative number"
            raise ValueError(msg)
        full_cwd = self._path(cwd)
        command = f"cd -- {shlex.quote(full_cwd)} && {shlex.join(argv)}"
        timeout_seconds = math.ceil(timeout) if timeout else 0
        response = await cast("SandboxBackendProtocol", self.backend).aexecute(
            command, timeout=timeout_seconds
        )
        return CommandResult(
            returncode=response.exit_code if response.exit_code is not None else -1,
            stdout=response.output,
        )
