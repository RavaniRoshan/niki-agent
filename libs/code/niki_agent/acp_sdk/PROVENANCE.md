# Vendored Deep Agents ACP package

`niki_agent/acp_sdk` is a copy of the `deepagents-acp` Python package, vendored
because the Agent Client Protocol integration is a product feature, not an
optional upstream dependency.

| Field | Value |
| --- | --- |
| Upstream project | <https://github.com/langchain-ai/deepagents> |
| Upstream package | `deepagents-acp` 0.0.12 |
| Copied from | this repository at commit `39a7a8c5` (`libs/acp/deepagents_acp`) |
| License | MIT — see `/LICENSE` and `/NOTICE` at the repository root |

## Changes made

- Import paths rewritten from `deepagents_acp.*` to `niki_agent.acp_sdk.*`.
- Imports of the SDK rewritten from `deepagents.*` to `niki_agent.sdk.*`.
- No behavioral changes.

The vendored tree is excluded from the product brand gate for the same reason
as `niki_agent/sdk`: the MIT notice travels with the original code.
