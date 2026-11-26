"""Hook contracts and compatibility dispatch.

This package contains two hook systems: Hooks v2 (current) and legacy hooks
(deprecated, removal September 1, 2026).

To write a new hook integration, use the v2 config format — see
`niki_agent.hooks.loading` for file locations and precedence, and
`niki_agent.hooks.models.config` + `niki_agent.hooks.models.wire`
for the schema and stdin payload shapes.

`niki_agent.hooks.legacy` exists only for backward compatibility; new
integrations should not target it.
"""

from niki_agent.hooks.legacy import (
    HOOK_SUBPROCESS_TIMEOUT,
    HOOK_TOOL_OUTPUT_LIMIT,
    _background_tasks,
    _dispatch_hook_sync as _dispatch_hook_sync,
    _load_hooks as _load_hooks,
    dispatch_hook,
    dispatch_hook_fire_and_forget,
    drain_pending_hooks,
    has_pending_hooks,
    subprocess as subprocess,
)

__all__ = [
    "HOOK_SUBPROCESS_TIMEOUT",
    "HOOK_TOOL_OUTPUT_LIMIT",
    "_background_tasks",
    "dispatch_hook",
    "dispatch_hook_fire_and_forget",
    "drain_pending_hooks",
    "has_pending_hooks",
]
