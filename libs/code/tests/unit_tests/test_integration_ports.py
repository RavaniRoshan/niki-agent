"""Tests for runtime and integration boundaries."""

from __future__ import annotations

import subprocess
import sys
from contextlib import AsyncExitStack
from typing import TYPE_CHECKING, Any, cast

import pytest

from niki_agent.adapters.runtime.langgraph_remote import LangGraphRemoteRuntime
from niki_agent.application.run_turn import RunTurnCoordinator
from niki_agent.domain.events import RunEvent, RunEventKind, TurnRequest

if TYPE_CHECKING:
    from collections.abc import AsyncIterator, Mapping, Sequence


class _FailingProviderStream:
    def __init__(self, error: Exception) -> None:
        self.error = error

    async def astream(
        self,
        graph_input: object,
        *,
        stream_mode: Sequence[str],  # noqa: ARG002
        subgraphs: bool,  # noqa: ARG002
        config: Mapping[str, object],  # noqa: ARG002
        context: Mapping[str, object] | None = None,  # noqa: ARG002
    ) -> AsyncIterator[object]:
        del graph_input
        raise self.error
        yield None  # Make this an async generator; execution never reaches this.


async def _failure_event(error: Exception) -> list[RunEvent]:
    runtime = LangGraphRemoteRuntime(_FailingProviderStream(error))
    request = TurnRequest("session-1", "turn-1", "implement change", "/workspace")
    return [event async for event in RunTurnCoordinator(runtime).stream_turn(request)]


@pytest.mark.asyncio
async def test_provider_error_maps_to_terminal_failure() -> None:
    error = OSError("provider connection lost")
    events = await _failure_event(error)

    assert [event.kind for event in events] == [
        RunEventKind.turn_started,
        RunEventKind.turn_failed,
    ]
    assert events[-1].payload == {"error": str(error), "recoverable": False}


@pytest.mark.asyncio
async def test_provider_timeout_emits_recoverable_failure() -> None:
    error = TimeoutError("provider request timed out")
    events = await _failure_event(error)

    assert [event.kind for event in events] == [
        RunEventKind.turn_started,
        RunEventKind.turn_failed,
    ]
    assert events[-1].payload == {"error": str(error), "recoverable": True}


@pytest.mark.asyncio
async def test_mcp_disconnect_is_visible_and_cleanup_is_idempotent():
    from niki_agent.mcp_tools import (
        MCPSessionManager,
        _classify_connect_failure,
    )

    class Client:
        close_count = 0

        async def close(self) -> None:
            self.close_count += 1

    backend_close_count = 0

    def close_backend() -> None:
        nonlocal backend_close_count
        backend_close_count += 1

    status, message = _classify_connect_failure(
        "workspace-tools", ConnectionError("MCP server disconnected"), redact=False
    )
    assert status == "error"
    assert "MCP server disconnected" in message

    client = Client()
    backend_stack = AsyncExitStack()
    backend_stack.callback(close_backend)
    manager = MCPSessionManager()
    manager.adopt(cast("Any", client), backend_stack)

    await manager.cleanup()
    await manager.cleanup()

    assert client.close_count == 1
    assert backend_close_count == 1
    assert manager.client is None


def test_domain_application_do_not_import_integration_adapters():
    script = """
import importlib
import sys

for module in ("niki_agent.domain", "niki_agent.application"):
    importlib.import_module(module)

forbidden = {"textual", "rich", "deepagents", "langchain", "langgraph"}
heavy = sorted(
    {name.split(".")[0] for name in sys.modules if name.split(".")[0] in forbidden}
)
if heavy:
    raise SystemExit(f"inner layers imported integration dependencies: {heavy}")
"""
    result = subprocess.run(
        [sys.executable, "-c", script], capture_output=True, text=True, check=False
    )
    assert result.returncode == 0, result.stdout + result.stderr


def test_integration_adapters_do_not_import_textual():
    script = """
import importlib
import sys

for module in (
    "niki_agent.mcp_tools",
    "niki_agent.plugins.adapters.hooks",
    "niki_agent.plugins.adapters.skills",
):
    importlib.import_module(module)

textual = sorted(
    name
    for name in sys.modules
    if name == "textual" or name.startswith("textual.")
)
if textual:
    raise SystemExit(f"integration adapters imported Textual: {textual}")
"""
    result = subprocess.run(
        [sys.executable, "-c", script], capture_output=True, text=True, check=False
    )
    assert result.returncode == 0, result.stdout + result.stderr
