# Archived packages

These packages are no longer part of the Niki Agent build. They were the rest of
the upstream Deep Agents monorepo; 1.0.0 vendors the two pieces Niki needs:

- `deepagents/` — its Python package became `libs/code/niki_agent/sdk`.
- `acp/` — its Python package became `libs/code/niki_agent/acp_sdk`.

`talon/` and `partners/` were never imported by `libs/code` at runtime and are
kept here for reference only. Their tests, locks, and CI jobs are retired; do not
re-enable them without a reason that outweighs the maintenance cost.

The full history is in Git. Nothing here is installed, linted, or tested.
