# Niki ACP Runtime Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route Niki’s ACP graph executions through `RunTurnCoordinator` while preserving the existing ACP server’s session, checkpoint, permission, and rendering behavior.

**Architecture:** Keep `deepagents-acp`’s protocol handler unchanged. Extend `ACPAgentRuntime` to accept the ACP graph input/configuration and forward original LangGraph chunks; route through it only inside Niki’s existing `_AutoGraph` adapter and `niki_runtime_scope()`. Keep one turn identity and validated sequence/status across ACP’s internal approval/resume loop.

**Tech Stack:** Python `>=3.12,<4.0`; LangGraph; existing Niki runtime contracts and `RunTurnCoordinator`; Textual-independent ACP adapter; pytest, Ruff, and `ty`.

**Spec:** `docs/superpowers/plans/2026-11-23-niki-agent-architecture.md` Task 4 ACP production-routing requirement; repository ACP protocol implementation in `libs/acp/deepagents_acp/server.py`; Niki ACP adapter in `libs/code/deepagents_code/acp.py`.

## Global Constraints

- Keep `libs/deepagents` unchanged; preserve `dcode` and `deepagents-code` entry point behavior while Niki evolves behind `niki`.
- Preserve the existing client/server ownership split: the client owns input, approvals, and presentation; the managed server owns graph execution, tools, workspace policy, and checkpoints.
- Keep LangGraph behind outer adapters; do not make domain or application code depend on LangGraph.
- Reuse the existing session/checkpoint, provider, workspace, MCP, hooks, skills, memory, goals, subagent, and QuickJS implementations where they already meet the contract.
- Do not add runtime dependencies unless a task demonstrates a requirement and records the maintenance reason.
- Preserve ACP multimodal input, native stream updates, ACP permission handling, and the server-owned session thread ID.

## Review Focus

- A text, image, audio, resource, or embedded-resource ACP prompt must reach the graph unchanged; pin this in Task 1 with `test_acp_runtime_forwards_native_input_and_chunks`.
- An approval resume must use the same session checkpoint and monotonically continue the same turn sequence; pin this in Task 1 with `test_acp_runtime_resumes_with_server_config_and_sequence`.
- A graph cancellation or consumer close must stop the graph stream and release the adapter’s active-turn state; pin this in Task 1 with `test_acp_runtime_cancel_stops_owned_graph_stream`.
- ACP’s existing top-level text, tool-call, plan, and permission updates must continue to come from native chunks; pin this in Task 2 with `test_niki_acp_prompt_preserves_native_protocol_updates`.
- `dcode` and `deepagents-code` must bypass coordinator routing; pin this in Task 2 with `test_auto_graph_bypasses_coordinator_outside_niki_scope`.

---

### Task 1: Make the ACP runtime adapter configurable and observable

**Files:**
- Modify: `libs/code/deepagents_code/adapters/runtime/acp.py`
- Test: `libs/code/tests/unit_tests/test_niki_entrypoints.py`

**Interfaces:**
- Consumes: `AgentRuntime`, `TurnRequest`, `ResumeRequest`, `RunEvent`, and `RunEventKind` from the existing runtime contracts.
- Produces: `ACPAgentRuntime(agent, *, on_chunk=None, stream_input=None, config=None, stream_mode=("messages", "updates", "custom"), subgraphs=True, stream_kwargs=None)`; it continues to implement `AgentRuntime`, and `on_chunk` receives each native graph chunk before event normalization.
- `stream_turn` uses `stream_input` when provided and keeps the current request-derived message fallback otherwise. `resume_turn` resumes with `Command(resume=request.decision)` on the same configured thread.

- [x] **Step 1: Write the failing forwarding test**

Add `test_acp_runtime_forwards_native_input_and_chunks`. Give a fake graph a multimodal `stream_input`, a full ACP config containing `configurable.thread_id` and `metadata.cwd`, `stream_mode=["messages", "updates"]`, `subgraphs=True`, and `stream_kwargs={"context": context}`. Assert the graph receives those exact values, the native message and update chunks are each passed to `on_chunk` in order, and the coordinator emits `turn_started`, the expected `text_delta`, and `turn_finished`.

```python
assert calls == [(native_input, config, ["messages", "updates"], True, {"context": context})]
assert received == native_chunks
assert [event.kind for event in events] == [
    RunEventKind.turn_started,
    RunEventKind.text_delta,
    RunEventKind.turn_finished,
]
```

- [x] **Step 2: Run the focused test and verify it fails**

Run: `cd libs/code && .venv/bin/pytest tests/unit_tests/test_niki_entrypoints.py::test_acp_runtime_forwards_native_input_and_chunks -q`
Expected: FAIL because `ACPAgentRuntime` does not yet accept execution options or report native chunks.

- [x] **Step 3: Implement configurable ACP graph execution**

Add the constructor options in `libs/code/deepagents_code/adapters/runtime/acp.py`. Pass the supplied graph input, full config with the request session ID enforced as `configurable.thread_id`, stream modes, subgraph setting, and extra graph kwargs to `agent.astream`. Invoke and await `on_chunk` before converting each graph chunk. Retain current defaults for existing callers.

- [x] **Step 4: Add the approval-resume continuity test**

Add `test_acp_runtime_resumes_with_server_config_and_sequence`. Stream an approval interrupt, use the approval event sequence and `TurnStatus.waiting_for_approval` in `ResumeRequest`, then resume with the same session and turn IDs. Assert `Command.resume` contains the supplied decision, the configured ACP thread ID remains the session ID, lifecycle sequence continues as `[3, 4, 5]`, and the resumed stream finishes.

```python
assert [event.kind for event in resumed] == [
    RunEventKind.session_resumed,
    RunEventKind.turn_started,
    RunEventKind.turn_finished,
]
assert [event.sequence for event in resumed] == [3, 4, 5]
assert graph_config == config
```

- [x] **Step 5: Add the owned-stream cancellation test**

Add `test_acp_runtime_cancel_stops_owned_graph_stream`. Use a fake graph that blocks in its owned stream. Start coordinator consumption, call `cancel_turn(session_id, turn_id)`, and assert the stream emits cancellation and clears its active-turn entry.

```python
assert events[-1].kind == RunEventKind.turn_cancelled
assert not runtime._active_tasks
```

- [x] **Step 6: Run adapter tests and checks**

Run: `cd libs/code && .venv/bin/pytest tests/unit_tests/test_niki_entrypoints.py tests/unit_tests/test_runtime_adapters.py tests/unit_tests/test_application_run_turn.py -q`
Expected: PASS.

Run Ruff check/format and `ty check` for the changed adapter/tests; expected: PASS.

- [x] **Step 7: Commit the adapter change**

```bash
git add libs/code/deepagents_code/adapters/runtime/acp.py libs/code/tests/unit_tests/test_niki_entrypoints.py
git commit -m "feat(code): preserve ACP graph streams in runtime adapter"
```

### Task 2: Route Niki ACP prompts through the coordinator

**Files:**
- Modify: `libs/code/deepagents_code/acp.py`
- Modify: `libs/code/tests/unit_tests/test_acp.py`
- Modify: `docs/niki/UPSTREAM_DIFF.md`
- Test: `libs/acp/tests/test_agent.py` (existing upstream ACP protocol regression suite)

**Interfaces:**
- Consumes: `ACPAgentRuntime` from Task 1; `RunTurnCoordinator`; `stream_native_chunks`; `TurnRequest`; `ResumeRequest`; `RunEvent`; and `niki_runtime_scope()` / `is_niki_runtime_enabled()`.
- Produces: `_RuntimeProgress(turn_id: str, sequence: int = 0, status: TurnStatus = TurnStatus.created)` scoped to one `AgentServerACP.prompt` invocation. `_AutoGraph.astream` runs its graph through `RunTurnCoordinator` only when that progress exists; it yields every original graph chunk to the unchanged base ACP prompt handler.

- [x] **Step 1: Write the failing scoped-routing test**

Add `test_niki_acp_prompt_preserves_native_protocol_updates` using an `AgentServerACP` graph factory and a fake graph that emits a top-level assistant message and a tool-call chunk. Run `prompt` inside `niki_runtime_scope()`. Assert the graph receives the ACP session ID as `configurable.thread_id`, the original message and tool-call chunks reach the ACP client as the expected message/tool updates, and `_RuntimeProgress` ends in `TurnStatus.completed` with an increasing sequence.

```python
assert [sequence for _kind, sequence, _status in progress_events] == [1, 2, 3]
assert progress_events[-1][2] == TurnStatus.completed
assert {"agent_message_chunk", "tool_call", "plan"} <= set(update_kinds)
```

- [x] **Step 2: Run the focused test and verify it fails**

Run: `cd libs/code && .venv/bin/pytest tests/unit_tests/test_acp.py::test_niki_acp_prompt_preserves_native_protocol_updates -q`
Expected: FAIL because the Niki-scoped progress context and coordinator route are not implemented yet.

- [x] **Step 3: Add ACP prompt-scoped runtime progress**

In `deepagents_code/acp.py`, create a `ContextVar[_RuntimeProgress | None]`. In `AgentServerACP.prompt`, set it to a fresh UUID and `TurnStatus.created` only when `is_niki_runtime_enabled()`; restore the token with the existing `_prompt` context in `finally`. Keep legacy entrypoints on the direct path.

- [x] **Step 4: Route native ACP graph calls through the coordinator**

In `_AutoGraph.astream`, preserve the existing Auto approval store update, prompt metadata, `CLIContextSchema`, and direct graph stream. When runtime progress is present, create `ACPAgentRuntime` over the underlying graph with the ACP input, config, `stream_mode`, `subgraphs`, and Auto context. Build `TurnRequest` for the initial message or `ResumeRequest` for `Command.resume`, using the same session/turn ID, current progress sequence/status, ACP text, and working directory. Stream native chunks through a one-item queue and `stream_native_chunks`; update progress from coordinator-validated events. Use the progress turn ID in `deepagents_code_user_prompt` metadata so runtime events and hook metadata share identity.

- [x] **Step 5: Add legacy-routing and resume compatibility tests**

Add `test_auto_graph_bypasses_coordinator_outside_niki_scope` and assert direct graph streaming still receives the original ACP input/context with no coordinator invocation. Add `test_niki_acp_approval_resume_keeps_runtime_turn_identity` and assert the approval request is made, the same runtime progress object and Auto turn ID survive `Command(resume=...)`, and the lifecycle sequence continues through completion.

```python
assert client.permission_requests == 1
assert len(progress_objects) == 1
assert [sequence for _kind, sequence in progress_events] == [1, 2, 3, 4, 5, 6]
assert resumed_context.turn_id == first_context.turn_id
```

- [x] **Step 6: Run ACP compatibility and runtime tests**

Run: `PYTHONPATH=libs/code:libs/acp:libs/deepagents libs/code/.venv/bin/pytest libs/code/tests/unit_tests/test_acp.py libs/code/tests/unit_tests/test_niki_entrypoints.py libs/code/tests/unit_tests/test_runtime_adapters.py libs/code/tests/unit_tests/test_application_run_turn.py libs/code/tests/unit_tests/test_main_acp_mode.py libs/code/tests/unit_tests/test_application_native_stream.py libs/code/tests/unit_tests/test_architecture_imports.py -q`
Expected: PASS (34 tests). Focused base-server compatibility coverage passes with `test_acp_agent_prompt_streams_visible_reasoning_in_block_order`, `test_acp_agent_tool_result_completes_tool_call`, `test_acp_agent_load_session_replays_tool_calls`, and `test_acp_agent_tool_call_chunk_starts_tool_call` (4 tests).

Known baseline limitation: `test_acp_agent_prompt_streams_text` times out on both this branch and the untouched main checkout when run alone; the full base ACP agent suite is not claimed as green.

- [x] **Step 7: Update the adapter note and run static checks**

Update `docs/niki/UPSTREAM_DIFF.md` to document that Niki ACP prompt graphs pass through the shared coordinator while the ACP server remains the owner of session IDs, checkpoint lifecycle, permission requests, and protocol rendering. Run Ruff check/format, `ty check`, and `git diff --check`; expected: PASS.

- [x] **Step 8: Commit the Niki ACP route**

```bash
git add libs/code/deepagents_code/acp.py libs/code/tests/unit_tests/test_acp.py docs/niki/UPSTREAM_DIFF.md
git commit -m "feat(code): route Niki ACP prompts through runtime port"
```

Commit: `0a05e8dc`.
Test follow-up: `a7342ce1` (`test(code): tighten ACP lifecycle assertions`).
