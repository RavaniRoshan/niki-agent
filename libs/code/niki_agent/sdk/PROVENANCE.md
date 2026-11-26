# Vendored Deep Agents SDK

`niki_agent/sdk` is a copy of the `deepagents` Python package, vendored so the
product no longer imports an upstream distribution at runtime.

| Field | Value |
| --- | --- |
| Upstream project | <https://github.com/langchain-ai/deepagents> |
| Upstream package | `deepagents` 0.7.21 |
| Copied from | this repository at commit `39a7a8c5` (`libs/deepagents/deepagents`) |
| License | MIT — see `/LICENSE` and `/NOTICE` at the repository root |

## Changes made

- Import paths rewritten from `deepagents.*` to `niki_agent.sdk.*`.
- Docstrings and cross-references that named the old package path were updated
  to the new one where they are rendered to users.
- Nothing in the agent loop, middleware, backend protocol, or prompt behavior
  was changed. The copy is behaviorally identical to the pinned release.

The vendored tree is excluded from the product brand gate on purpose: MIT
requires the original copyright notice to survive, and rewriting upstream
prose would make the provenance harder to audit. Product-facing strings live
outside this directory.
