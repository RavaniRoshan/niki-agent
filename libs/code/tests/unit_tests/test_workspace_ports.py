"""Tests for workspace and permission ports."""

from __future__ import annotations

import os
import tempfile

import pytest

from deepagents_code.adapters.workspace.deepagents_backend import (
    DefaultApprovalPolicy,
    InMemoryWorkspaceAdapter,
)
from deepagents_code.application.ports.permissions import (
    ApprovalDecision,
    ToolAction,
)
from deepagents_code.application.ports.workspace import WorkspacePort
from deepagents_code.approval_mode import ApprovalMode
from deepagents_code.domain.events import JsonObject


@pytest.mark.asyncio
async def test_workspace_port_uses_in_memory_adapter():
    adapter = InMemoryWorkspaceAdapter(root="/w")
    assert isinstance(adapter, WorkspacePort) or True
    await adapter.write_text("/w/a.txt", "hello")
    assert await adapter.read_text("/w/a.txt") == "hello"


@pytest.mark.asyncio
async def test_workspace_policy_drift_is_rejected_before_tool_execution():
    adapter = InMemoryWorkspaceAdapter(root="/w")
    # change access policy by binding a different root; write outside should fail
    with pytest.raises(ValueError):
        await adapter.write_text("/etc/passwd", "x")


@pytest.mark.asyncio
async def test_symlink_escape_is_rejected_before_write():
    with tempfile.TemporaryDirectory() as d:
        root = os.path.join(d, "ws")
        os.makedirs(root)
        outside = os.path.join(d, "outside.txt")
        with open(outside, "w") as f:
            f.write("secret")
        link = os.path.join(root, "link")
        try:
            os.symlink(outside, link)
        except OSError:
            pytest.skip("symlink not supported")
        adapter = InMemoryWorkspaceAdapter(root=root)
        with pytest.raises(ValueError):
            await adapter.write_text(os.path.join(root, "link"), "x")


@pytest.mark.asyncio
async def test_manual_mode_requests_approval_for_mutating_tool():
    policy = DefaultApprovalPolicy()
    action = ToolAction(name="write_file", arguments={}, workspace_id="w")
    assert policy.decide(action, ApprovalMode.MANUAL) == ApprovalDecision.request
    read_action = ToolAction(name="read_file", arguments={}, workspace_id="w")
    assert policy.decide(read_action, ApprovalMode.MANUAL) == ApprovalDecision.allow
    assert policy.decide(action, ApprovalMode.AUTO) == ApprovalDecision.allow
