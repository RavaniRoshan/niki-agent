# Design independence

Niki Agent is a standalone product. It is not a configuration layer over someone
else's runtime, and it does not need another agent package installed for it to
work. This page lists what that means in practice.

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

The agent runtime, the tool registry, the approval policy, and the
editor-protocol integration all live in this repository, under
`libs/code/niki_agent/`. Nothing imports an agent framework at runtime.

The components that are included rather than written here keep their MIT notices
in [`NOTICE`](../../NOTICE), with a `LICENSE` and a `PROVENANCE.md` beside each
directory so the copy stays auditable. Attribution is a legal requirement and is
never removed by branding work.

## One state directory

Everything Niki writes lives under `~/.niki`:

```
~/.niki/
  config.toml        # providers, theme, policy
  sessions/          # transcripts and checkpoints
  skills/            # user skills
  cache/
```

`NIKI_HOME` moves the whole tree, which is what makes a portable install and a
test harness possible. Credentials are read from provider environment variables;
they are never copied into this tree.

## One environment namespace

Every `NIKI_*` variable means the same thing on Linux, macOS, and Windows, and is
documented in one place. Provider API keys keep their vendor names so an existing
shell profile works unchanged.

## One release pipeline

Wheels are built here, three platform archives are assembled here, and
`SHA256SUMS` is published with every release. The installers verify the checksum
before installing, and CI pins every action by commit.

## Compatibility

Niki reads a small number of legacy names so an upgrade does not break a working
setup. Those paths are implementation details, they are documented in the code
that implements them, and they are scheduled for removal at a major version.
