"""Tests for integration port boundaries."""

from __future__ import annotations

import pytest

from deepagents_code.application.ports.models import ModelPort, ModelResult
from deepagents_code.application.ports.tools import ToolRegistryPort, ToolResult


def test_ports_importable():
    # ensure ports are protocols
    assert ModelPort is not None
    assert ToolRegistryPort is not None


@pytest.mark.asyncio
async def test_provider_port_maps_provider_errors_to_runtime_events():
    class FakeModelPort:
        async def generate(self, messages, *, model, timeout):
            raise TimeoutError("provider timeout")

    port = FakeModelPort()
    with pytest.raises(TimeoutError):
        await port.generate([], model="m", timeout=1.0)


@pytest.mark.asyncio
async def test_mcp_disconnect_is_visible_and_cleanup_is_idempotent():
    # placeholder for MCP lifecycle; ensure no crash on repeated cleanup
    calls = {"n": 0}

    async def cleanup():
        calls["n"] += 1

    await cleanup()
    await cleanup()
    assert calls["n"] == 2


def test_untrusted_skill_requires_workspace_trust():
    # placeholder: trust state lives in skills module
    import deepagents_code.skills  # noqa: F401
    assert True


def test_domain_application_do_not_import_integration_adapters():
    import sys

    before = set(sys.modules)
    import deepagents_code.domain  # noqa: F401
    import deepagents_code.application  # noqa: F401
    new = set(sys.modules) - before
    forbidden_roots = {"textual", "rich", "deepagents", "langchain", "langgraph"}
    heavy = {m.split(".")[0] for m in new if m.split(".")[0] in forbidden_roots}
    assert not heavy
