# Vendored from the QuickJS middleware in LangChain's Deep Agents project
# (https://github.com/langchain-ai/deepagents, `libs/partners/quickjs`),
# version 0.3.8. MIT licensed by LangChain, Inc.; see LICENSE and
# PROVENANCE.md in this directory. Import paths were rewritten; the
# implementation is unchanged.

"""Persistent JavaScript REPL middleware for the agent."""

from niki_agent._vendor.quickjs._ptc import PTCAuthorizer, PTCDecision, PTCOption
from niki_agent._vendor.quickjs._subagent import (
    SUBAGENT_STREAM_EVENT_TYPE,
    SubagentCompleteEvent,
    SubagentErrorEvent,
    SubagentStartEvent,
    SubagentStreamEvent,
)
from niki_agent._vendor.quickjs.middleware import CodeInterpreterMiddleware

__all__ = [
    "SUBAGENT_STREAM_EVENT_TYPE",
    "CodeInterpreterMiddleware",
    "PTCAuthorizer",
    "PTCDecision",
    "PTCOption",
    "SubagentCompleteEvent",
    "SubagentErrorEvent",
    "SubagentStartEvent",
    "SubagentStreamEvent",
]
