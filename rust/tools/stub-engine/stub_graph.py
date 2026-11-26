"""Deterministic graph used only to capture the LangGraph SSE protocol."""

from __future__ import annotations

from typing import Any

from langchain_core.language_models.fake_chat_models import GenericFakeChatModel
from langchain_core.messages import AIMessage
from langgraph.graph import END, START, MessagesState, StateGraph


def respond(state: MessagesState) -> dict[str, Any]:
    model = GenericFakeChatModel(
        messages=iter([AIMessage(content="Hello from the Niki protocol fixture.")])
    )
    chunks = list(model.stream(state["messages"]))
    reply = chunks[0]
    for chunk in chunks[1:]:
        reply += chunk
    return {"messages": [reply]}


builder = StateGraph(MessagesState)
builder.add_node("respond", respond)
builder.add_edge(START, "respond")
builder.add_edge("respond", END)
graph = builder.compile()
