# `libs/partners`

One package remains: `quickjs`, the JS interpreter the agent imports.

It is built from this repository because its 0.3.8 line carries the PTC API the
agent imports; the published 0.3.8 does not. It has its own `pyproject.toml`,
`Makefile`, and `uv.lock`.

Repository-wide rules live in the root [`AGENTS.md`](../../AGENTS.md). Follow the
"Warnings are errors" policy cited by its `pyproject.toml`.

Do not re-add a partner package to CI, release configuration, or dependency
groups without a reason that outweighs the maintenance cost. The release
pipeline builds only `niki-agent`.
