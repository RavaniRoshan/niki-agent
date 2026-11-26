"""Programmatic tool calling (PTC) support for `CodeInterpreterMiddleware`.

PTC exposes the agent's LangChain tools inside the JavaScript REPL as
`tools.<camelCaseName>(input)` async functions. Instead of issuing
N serial tool calls, the model writes one `eval` that loops / parallelises
/ chains tools in-code:

    const [a, b] = await Promise.all([
        tools.search({query: "foo"}),
        tools.search({query: "bar"}),
    ]);

Two pieces live here:

- filtering — turn the live agent toolset into the subset exposed to PTC
- prompt rendering — render a short TS-ish API-reference block describing
    each exposed tool, so the model knows the call shape

The host-function bridge that actually invokes each tool lives in
`_repl.py` next to the rest of the context wiring.
"""

from __future__ import annotations

import logging
from collections.abc import Awaitable, Callable
from typing import TYPE_CHECKING, Any, Literal

from langchain.tools import ToolRuntime
from langchain_core.tools import BaseTool

from niki_agent._vendor.quickjs import _prompt

if TYPE_CHECKING:
    from collections.abc import Sequence


PTCOption = list[str | BaseTool]
PTCDecision = Literal["allow", "deny", "requires_direct_approval"]
PTCAuthorizer = Callable[
    [BaseTool, dict[str, Any], ToolRuntime], Awaitable[PTCDecision]
]

logger = logging.getLogger(__name__)

_RESERVED_SUBAGENT_TASK_NAME = "task"

_TASK_IN_PTC_MSG = (
    "The subagent `task` tool cannot be exposed via `ptc`. It is always "
    "available as the top-level `task()` global inside the REPL (with "
    "`subagentType`, `label`, and `responseSchema` support); exposing it through "
    "the `tools.*` namespace would create a second, conflicting dispatch path "
    'that drops `responseSchema`. Remove "task" from `ptc`.'
)


def filter_tools_for_ptc(
    tools: Sequence[BaseTool],
    config: PTCOption,
    *,
    self_tool_name: str,
) -> list[BaseTool]:
    """Return the subset of `tools` exposed inside the REPL.

    `self_tool_name` is the REPL's own tool name; it is *always* excluded
    to prevent the model from recursing `tools.eval("tools.eval(...)")`.
    If the model wants a nested eval, it can just write nested code in one
    call — that's the whole point of PTC.

    `config` is allowlist-only:

    - `str` entries: expose matching tool names from `tools`.
    - `BaseTool` entries: expose those tools directly (minus
        `self_tool_name`).

    Mixed lists are supported and merged. Explicit `BaseTool` entries
    are included first, then name-matched agent tools are appended.
    Duplicate tool names are deduplicated.

    The subagent `task` tool is reserved and may not appear in `config`
    (by name or instance) — it is always available as the `task()` global,
    so a `tools.task` PTC variant would be a conflicting, degraded duplicate.
    A `"task"` entry raises `ValueError`.

    Warning:
        PTC tool calls execute through the REPL bridge. Applications with
        approval rules must inject a server-owned authorizer into
        `CodeInterpreterMiddleware`; the bridge invokes it before each host
        call and denies calls if authorization fails.
    """
    if isinstance(config, list):
        explicit_tools: list[BaseTool] = []
        allow_names: set[str] = set()
        for entry in config:
            if isinstance(entry, BaseTool):
                if entry.name == _RESERVED_SUBAGENT_TASK_NAME:
                    raise ValueError(_TASK_IN_PTC_MSG)
                if entry.name != self_tool_name:
                    explicit_tools.append(entry)
                continue
            if isinstance(entry, str):
                if entry == _RESERVED_SUBAGENT_TASK_NAME:
                    raise ValueError(_TASK_IN_PTC_MSG)
                allow_names.add(entry)
                continue
            msg = "ptc list entries must be str or BaseTool"
            raise TypeError(msg)
        selected = [
            *explicit_tools,
            *[t for t in tools if t.name != self_tool_name and t.name in allow_names],
        ]
        deduped: list[BaseTool] = []
        seen_names: set[str] = set()
        for tool in selected:
            if tool.name in seen_names:
                continue
            seen_names.add(tool.name)
            deduped.append(tool)
        selected = deduped
        _raise_on_invalid_ptc_tools(selected)
        return selected
    msg = (
        "Unsupported `ptc` config type. "
        "Use a list of tool names, list of BaseTool instances, or disable PTC."
    )
    raise TypeError(msg)


def to_camel_case(name: str) -> str:
    """Convert `snake_case` / `kebab-case` → `camelCase`."""
    return _prompt.to_camel_case(name)


def is_valid_js_identifier(name: str) -> bool:
    """Return whether `name` is a valid JavaScript identifier."""
    return _prompt.is_valid_js_identifier(name)


def is_valid_ptc_tool_name(name: str) -> bool:
    """Return whether a tool can be exposed as `tools.<camelCaseName>`."""
    return _prompt.is_valid_ptc_tool_name(name)


def _raise_on_invalid_ptc_tools(tools: Sequence[BaseTool]) -> None:
    for tool in tools:
        camel = to_camel_case(tool.name)
        if is_valid_js_identifier(camel):
            continue
        msg = (
            f"PTC tool name {tool.name!r} cannot be exposed as JavaScript "
            f"identifier {camel!r}. Tool names must map to "
            "`/^[A-Za-z_$][A-Za-z0-9_$]*$/`."
        )
        raise ValueError(msg)


def render_ptc_prompt(tools: Sequence[BaseTool], *, tool_name: str = "eval") -> str:
    """Build the `tools` namespace section of the system prompt."""
    if not tools:
        return ""
    _raise_on_invalid_ptc_tools(tools)
    return _prompt.render_ptc_prompt(tools, tool_name=tool_name)


class PTCToolGate:
    """Run a server-injected approval decision before a PTC tool invocation.

    The authorizer owns policy resolution (including live approval-mode and
    workspace checks). QuickJS treats it as a trusted server callback; guest
    JavaScript, tool arguments, and repository text cannot replace it.
    """

    def __init__(self, authorizer: PTCAuthorizer) -> None:
        """Bind the server-owned authorization callback for this middleware."""
        self._authorizer = authorizer

    async def authorize(
        self,
        tool: BaseTool,
        arguments: dict[str, Any],
        runtime: ToolRuntime,
    ) -> PTCDecision:
        """Resolve one tool call, failing closed on callback errors or bad data."""
        try:
            decision = await self._authorizer(tool, arguments, runtime)
        except Exception:  # noqa: BLE001 — an authorizer that raises must deny the call
            logger.warning(
                "PTC authorization failed for tool %r; denying call",
                tool.name,
                exc_info=True,
            )
            return "deny"
        if decision in ("allow", "deny", "requires_direct_approval"):
            return decision
        logger.warning(
            "PTC authorization returned an invalid decision for tool %r; denying call",
            tool.name,
        )
        return "deny"
