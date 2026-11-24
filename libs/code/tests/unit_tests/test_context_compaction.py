"""Goal notice behavior at context compaction boundaries."""

from __future__ import annotations

import asyncio
from types import SimpleNamespace

from langchain_core.messages import HumanMessage

from deepagents_code.goal_state_notice import (
    build_goal_state_notice,
    goal_state_notice_info,
)
from deepagents_code.goal_tools import GoalToolsMiddleware


def _request_with_writer(
    *,
    messages: list[HumanMessage],
    state: dict[str, object],
    events: list[dict[str, object]],
) -> SimpleNamespace:
    request = SimpleNamespace(
        messages=messages,
        state=state,
        runtime=SimpleNamespace(stream_writer=events.append),
    )

    def attach(target: SimpleNamespace) -> SimpleNamespace:
        def override(**changes: object) -> SimpleNamespace:
            result = SimpleNamespace(**vars(target))
            result.__dict__.update(changes)
            return attach(result)

        target.override = override
        return target

    return attach(request)


def test_compaction_preserves_latest_goal_notice():
    stale_notice = build_goal_state_notice(
        {"_goal_objective": "old objective", "_goal_status": "active"}
    )
    messages = [
        HumanMessage(content="start"),
        stale_notice,
        HumanMessage(content="recent work"),
    ]
    state = {
        "_goal_objective": "current objective",
        "_goal_status": "blocked",
        "_goal_status_note": "waiting for API access",
        "messages": messages,
        "_summarization_event": {"cutoff_index": 2},
    }
    request = SimpleNamespace(messages=messages, state=state)

    def override(**changes: object) -> SimpleNamespace:
        result = SimpleNamespace(messages=messages, state=state)
        for name, value in changes.items():
            setattr(result, name, value)
        result.override = override
        return result

    request.override = override
    projected = GoalToolsMiddleware._request_with_goal_notice(request)

    assert len(projected.messages) == len(messages) + 1
    latest = projected.messages[-1]
    info = goal_state_notice_info(latest)
    assert info is not None
    assert (
        info["state_fingerprint"]
        != goal_state_notice_info(stale_notice)["state_fingerprint"]
    )
    assert "current objective" in latest.content
    assert "waiting for API access" in latest.content


def test_context_compaction_emits_only_when_cutoff_advances():
    events: list[dict[str, object]] = []
    messages = [HumanMessage(content="first"), HumanMessage(content="second")]
    request = _request_with_writer(
        messages=messages,
        state={"_summarization_event": {"cutoff_index": 1}},
        events=events,
    )

    async def handler(_request: object) -> SimpleNamespace:
        await asyncio.sleep(0)
        return SimpleNamespace(
            command=SimpleNamespace(
                update={"_summarization_event": {"cutoff_index": 2}}
            )
        )

    asyncio.run(GoalToolsMiddleware().awrap_model_call(request, handler))

    assert events == [{"type": "niki_context_compacted", "cutoff_index": 2}]


def test_context_compaction_does_not_emit_for_repeated_or_invalid_cutoff():
    for cutoff in (1, True, 3):
        events: list[dict[str, object]] = []
        messages = [HumanMessage(content="one"), HumanMessage(content="two")]
        request = _request_with_writer(
            messages=messages,
            state={"_summarization_event": {"cutoff_index": 1}},
            events=events,
        )

        async def handler(
            _request: object, *, next_cutoff: object = cutoff
        ) -> SimpleNamespace:
            await asyncio.sleep(0)
            return SimpleNamespace(
                command=SimpleNamespace(
                    update={"_summarization_event": {"cutoff_index": next_cutoff}}
                )
            )

        asyncio.run(GoalToolsMiddleware().awrap_model_call(request, handler))
        assert events == []
