"""Model provider port."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol, Sequence

from deepagents_code.domain.events import JsonObject


@dataclass(frozen=True, slots=True)
class ModelResult:
    text: str
    tokens: int | None = None
    cost_usd: float | None = None


class ModelPort(Protocol):
    async def generate(
        self, messages: Sequence[JsonObject], *, model: str, timeout: float
    ) -> ModelResult:
        ...
