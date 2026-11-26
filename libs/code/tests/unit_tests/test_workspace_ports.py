"""Tests for workspace and permission ports."""

from __future__ import annotations

import shlex
from typing import TYPE_CHECKING, Any, cast

import pytest
from deepagents.backends.protocol import ExecuteResponse, ReadResult, WriteResult

from deepagents_code.adapters.permissions.hitl import DefaultApprovalPolicy
from deepagents_code.adapters.workspace.deepagents_backend import (
    DeepAgentsWorkspaceAdapter,
    InMemoryWorkspaceAdapter,
)
from deepagents_code.application.ports.permissions import (
    ApprovalDecision,
    ToolAction,
)
from deepagents_code.application.ports.workspace import WorkspacePort
from deepagents_code.approval_mode import ApprovalMode

if TYPE_CHECKING:
    from pathlib import Path


def _create_symlink(target: Path, link: Path) -> None:
    """Create a link synchronously for the async security test."""
    link.symlink_to(target)


class FakeBackend:
    """Async backend double that records calls at the adapter boundary."""

    def __init__(self) -> None:
        self.writes: list[tuple[str, str]] = []
        self.commands: list[tuple[str, int | None]] = []
        self.read_result = ReadResult(
            file_data={"content": "hello", "encoding": "utf-8"}
        )
        self.write_result = WriteResult()

    async def aread(self, path: str) -> ReadResult:
        del path
        return self.read_result

    async def awrite(self, path: str, content: str) -> WriteResult:
        self.writes.append((path, content))
        return self.write_result

    async def aexecute(
        self,
        command: str,
        *,
        timeout: int | None = None,  # noqa: ASYNC109
    ) -> ExecuteResponse:
        self.commands.append((command, timeout))
        return ExecuteResponse(output="ok", exit_code=7)


async def test_workspace_port_uses_in_memory_adapter() -> None:
    adapter = InMemoryWorkspaceAdapter(root="/w")
    assert isinstance(adapter, WorkspacePort)

    await adapter.write_text("/w/a.txt", "hello")

    assert await adapter.read_text("/w/a.txt") == "hello"


async def test_backend_adapter_uses_async_backend_results(tmp_path) -> None:
    root = tmp_path / "workspace"
    root.mkdir()
    backend = cast("Any", FakeBackend())
    adapter = DeepAgentsWorkspaceAdapter(
        backend, workspace_id="workspace-1", workspace_root=str(root)
    )

    assert await adapter.read_text(str(root / "a.txt")) == "hello"
    await adapter.write_text(str(root / "a.txt"), "updated")
    result = await adapter.run_command(
        ["echo", "hello world"], cwd=str(root), timeout=0.2
    )

    assert backend.writes == [(str(root / "a.txt"), "updated")]
    assert backend.commands == [
        (f"cd -- {shlex.quote(str(root))} && echo 'hello world'", 1)
    ]
    assert (result.returncode, result.stdout, result.stderr) == (7, "ok", "")


async def test_workspace_path_escape_is_rejected_before_backend_write(tmp_path) -> None:
    root = tmp_path / "workspace"
    root.mkdir()
    backend = cast("Any", FakeBackend())
    adapter = DeepAgentsWorkspaceAdapter(
        backend, workspace_id="workspace-1", workspace_root=str(root)
    )

    with pytest.raises(ValueError, match="escapes workspace"):
        await adapter.write_text(str(tmp_path / "outside.txt"), "x")

    assert backend.writes == []


def test_backend_adapter_rejects_an_empty_workspace_root() -> None:
    with pytest.raises(ValueError, match="workspace_root"):
        DeepAgentsWorkspaceAdapter(
            cast("Any", FakeBackend()), workspace_id="workspace-1", workspace_root=""
        )


async def test_command_cwd_escape_is_rejected_before_backend_execution(
    tmp_path,
) -> None:
    root = tmp_path / "workspace"
    root.mkdir()
    backend = cast("Any", FakeBackend())
    adapter = DeepAgentsWorkspaceAdapter(
        backend, workspace_id="workspace-1", workspace_root=str(root)
    )

    with pytest.raises(ValueError, match="escapes workspace"):
        await adapter.run_command(
            ["touch", str(tmp_path / "outside.txt")],
            cwd=str(tmp_path),
            timeout=1,
        )

    assert backend.commands == []


async def test_boolean_timeout_is_rejected_before_backend_execution(tmp_path) -> None:
    root = tmp_path / "workspace"
    root.mkdir()
    backend = cast("Any", FakeBackend())
    adapter = DeepAgentsWorkspaceAdapter(
        backend, workspace_id="workspace-1", workspace_root=str(root)
    )

    with pytest.raises(ValueError, match="timeout"):
        await adapter.run_command(["true"], cwd=str(root), timeout=True)

    assert backend.commands == []


async def test_backend_failures_are_not_reported_as_success(tmp_path) -> None:
    root = tmp_path / "workspace"
    root.mkdir()
    backend = cast("Any", FakeBackend())
    adapter = DeepAgentsWorkspaceAdapter(
        backend, workspace_id="workspace-1", workspace_root=str(root)
    )
    backend.read_result = ReadResult(error="read denied")
    backend.write_result = WriteResult(error="write denied")

    with pytest.raises(OSError, match="read denied"):
        await adapter.read_text(str(root / "a.txt"))
    with pytest.raises(OSError, match="write denied"):
        await adapter.write_text(str(root / "a.txt"), "x")


async def test_symlink_escape_is_rejected_before_backend_write(tmp_path) -> None:
    root = tmp_path / "workspace"
    root.mkdir()
    outside = tmp_path / "outside.txt"
    outside.write_text("secret")
    link = root / "link"
    try:
        _create_symlink(outside, link)
    except OSError:
        pytest.skip("symlink not supported")
    backend = cast("Any", FakeBackend())
    adapter = DeepAgentsWorkspaceAdapter(
        backend, workspace_id="workspace-1", workspace_root=str(root)
    )

    with pytest.raises(ValueError, match="escapes workspace"):
        await adapter.write_text(str(link), "x")

    assert backend.writes == []


async def test_in_memory_workspace_rejects_outside_path() -> None:
    adapter = InMemoryWorkspaceAdapter(root="/w")

    with pytest.raises(ValueError, match="escapes workspace"):
        await adapter.write_text("/etc/passwd", "x")


def test_manual_mode_requests_approval_for_configured_tool() -> None:
    policy = DefaultApprovalPolicy(
        workspace_id="workspace-1", gated_tool_names={"write_file"}
    )
    write_action = ToolAction(
        name="write_file", arguments={}, workspace_id="workspace-1"
    )
    read_action = ToolAction(name="read_file", arguments={}, workspace_id="workspace-1")

    assert policy.decide(write_action, ApprovalMode.MANUAL) == ApprovalDecision.request
    assert policy.decide(read_action, ApprovalMode.MANUAL) == ApprovalDecision.allow
    assert policy.decide(write_action, ApprovalMode.AUTO) == ApprovalDecision.allow
    assert policy.decide(write_action, ApprovalMode.YOLO) == ApprovalDecision.allow


def test_workspace_identity_mismatch_is_denied() -> None:
    policy = DefaultApprovalPolicy(
        workspace_id="workspace-1", gated_tool_names={"write_file"}
    )
    action = ToolAction(name="write_file", arguments={}, workspace_id="workspace-2")

    assert policy.decide(action, ApprovalMode.MANUAL) == ApprovalDecision.deny
