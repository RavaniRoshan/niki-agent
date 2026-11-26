"""Architecture import-guard tests.

Ensures inner layers (domain, application) do not pull heavy dependencies.
"""

import sys


def test_domain_and_application_import_no_heavy_deps():
    before = set(sys.modules)
    import niki_agent.application
    import niki_agent.application.native_stream
    import niki_agent.application.ports
    import niki_agent.domain

    new = set(sys.modules) - before
    forbidden = {"textual", "rich", "deepagents", "langchain", "langgraph"}
    heavy = {m.split(".")[0] for m in new if m.split(".")[0] in forbidden}
    assert not heavy, f"inner layer pulled heavy deps: {heavy}"
