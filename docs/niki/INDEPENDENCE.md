# Independent by design

Niki Agent is a standalone product. Everything the agent *does* is implemented
in this repository: the loop, the tool registry, the approval policy, and the
editor-protocol integration. Niki depends on model-provider client libraries and
a graph runtime, the same way any application depends on its framework — but it
does not depend on another coding agent's package to run.

## One package, one command

| Surface | Value |
| --- | --- |
| Python package | `niki_agent` |
| Distribution | `niki-agent` |
| Command | `niki` |
| Environment prefix | `NIKI_*` |
| State directory | `~/.niki` |
| On-disk project config | `.niki/` |

## One agent loop

The runtime, the tool registry, the approval policy, and the editor-protocol
integration all live under `libs/code/niki_agent/`. The vendored SDK and ACP
runtime are part of the package, so installing `niki-agent` is enough — there is
no second distribution to install and nothing to keep in version lockstep.

The components that are included rather than written here keep their MIT notices
in [`NOTICE`](../../NOTICE), with a `LICENSE` and a `PROVENANCE.md` beside each
directory so the copy stays auditable. Attribution is a legal requirement and is
never removed by branding work.

## One state directory

Everything Niki writes lives under `~/.niki`:

```
~/.niki/
  config.toml            # providers, theme, policy
  .env                   # project-independent environment
  .mcp.json              # MCP servers
  hooks.json             # lifecycle hooks
  plugins/               # installed plugins
  agent/
    skills/              # user skills
  conversation_history/  # offloaded transcripts
  .state/
    auth.json            # credentials entered through /auth (0600)
    sessions.db          # threads, checkpoints, message history
    history.jsonl        # prompt history
    cache/               # model catalog and pricing cache
    mcp-tokens/          # MCP OAuth tokens
    locks/               # cross-process locks
```

`NIKI_HOME` moves the whole tree, which is what makes a portable install and a
test harness possible. Credentials you paste into `/auth` are written to
`~/.niki/.state/auth.json` with mode `0600`; credentials supplied as environment
variables are read from the environment and never copied into the tree.

A project can also carry its own `.niki/` directory for skills, agents, hooks,
and MCP configuration, so a repository can ship its Niki setup with it.

## One environment namespace

Every `NIKI_*` variable means the same thing on Linux, macOS, and Windows.
Provider API keys keep their vendor names (`ANTHROPIC_API_KEY`,
`OPENAI_API_KEY`, and so on) so an existing shell profile works unchanged.

## One release pipeline

Wheels are built here, three platform archives are assembled here, and
`SHA256SUMS` is published with every release. The installers verify the checksum
before installing, and the macOS archive is built from the same pure-Python wheel
as the others.

## Upgrades

Niki reads a small number of earlier on-disk names so an upgrade does not break a
working setup. Those paths are implementation details, they are documented in the
code that implements them, and they are scheduled for removal at a major version.
