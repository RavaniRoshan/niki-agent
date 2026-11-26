from __future__ import annotations


def test_import_main_module() -> None:
    from niki_agent.acp_sdk import __main__  # noqa: F401
