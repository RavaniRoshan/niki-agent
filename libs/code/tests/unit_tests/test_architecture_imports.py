"""Architecture import-guard tests.

Ensures inner layers (domain, application) do not pull heavy dependencies.
"""

import sys


def test_domain_and_application_import_no_heavy_deps():
    before = set(sys.modules)
    import deepagents_code.application
    import deepagents_code.application.native_stream
    import deepagents_code.application.ports
    import deepagents_code.domain

    new = set(sys.modules) - before
    forbidden = {"textual", "rich", "deepagents", "langchain", "langgraph"}
    heavy = {m.split(".")[0] for m in new if m.split(".")[0] in forbidden}
    assert not heavy, f"inner layer pulled heavy deps: {heavy}"
