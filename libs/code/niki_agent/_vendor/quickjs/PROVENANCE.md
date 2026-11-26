# Vendored QuickJS middleware

`niki_agent/_vendor/quickjs` is a copy of the `langchain-quickjs` Python package,
vendored because the agent needs its PTC (programmatic tool calling) API, which
the published releases in the supported range do not carry. Keeping the copy in
the product makes the wheel self-contained: nothing imports an upstream
distribution at runtime.

| Field | Value |
| --- | --- |
| Upstream project | <https://github.com/langchain-ai/deepagents> |
| Upstream package | `langchain-quickjs` 0.3.8 |
| Copied from | this repository at commit `a83b9111` (`libs/partners/quickjs/langchain_quickjs`) |
| License | MIT — see `LICENSE` in this directory and `/NOTICE` at the repository root |

## Changes made

- Import paths rewritten from `langchain_quickjs.*` to
  `niki_agent._vendor.quickjs.*`, including the docstring example in
  `middleware.py`.
- The package docstring in `__init__.py` no longer names the upstream package.
- Nothing else changed. The REPL, PTC gate, snapshot format, prompt assembly,
  and subagent bridge are behaviorally identical to the 0.3.8 copy.

The vendored tree is excluded from ruff and ty, like the other vendored trees:
MIT requires the original code to survive, and a diff against this file stays
auditable only while it is untouched.
