"""Warning-toggle definitions for the notification hub's settings section."""

from __future__ import annotations

from niki_agent.approval_mode import YOLO_WARNING_KEY
from niki_agent.cold_cache import COLD_CACHE_WARNING_KEY

# Warning keys and their user-facing labels.
# Checked = warning is shown (not suppressed). Unchecked = suppressed.
WARNING_TOGGLES: list[tuple[str, str]] = [
    (COLD_CACHE_WARNING_KEY, "Warn before expensive cold prompt-cache turns"),
    ("ripgrep", "Warn when ripgrep is not installed"),
    ("tavily", "Warn when TAVILY_API_KEY is not set (web search)"),
    (YOLO_WARNING_KEY, "Warn when YOLO mode is active (no approval review)"),
]
