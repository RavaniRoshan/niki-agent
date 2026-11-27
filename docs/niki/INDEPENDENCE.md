# Independent by design

Niki Agent is a standalone product. The turn loop, the tool registry, the
approval policy, the provider adapters, the MCP client, hooks, skills and the
terminal UI are all written in this repository and compiled into one binary.
Niki does not call another agent service and does not import an agent framework
to run.

## One binary, one command

| Surface | Value |
| --- | --- |
| Binary | `niki` |
| Crate root | `rust/crates/niki` |
| Command | `niki` |
| Environment prefix | `NIKI_*` |
| State directory | `~/.niki` |
| On-disk project config | `.niki/` |

`niki` opens the full-screen app; `niki -n "prompt"` runs one prompt and exits.
The same executable serves both.

## One agent loop

The loop, the tools, the policy and the presentation are separate crates in one
workspace, but they are linked together at build time. There is no second
distribution to install and no version lockstep to maintain. The ACP editor
server is a host around the same loop, not a second agent.

The provider adapters speak the two HTTP wire protocols directly: OpenAI Chat
Completions and Anthropic Messages. A provider is an endpoint plus a key
environment variable, not a separate runtime package. Adding a gateway is a
configuration block, not a dependency.

Third-party crates keep their own licences, recorded in `NOTICE`. That
attribution is a legal requirement and is not changed by branding.

## One state directory

Everything Niki writes lives under `NIKI_HOME` when set, otherwise `~/.niki`:

```
~/.niki/
  config.toml            # providers, default model, theme, MCP servers
  hooks.json             # user lifecycle hooks
  skills/                # user skills
  sessions.db            # SQLite transcript store
```

`NIKI_HOME` moves the whole tree, which is what makes a portable install and a
test harness possible. The binary reads `config.toml`; it does not write it.
Provider keys are read from the environment variable named by `api_key_env` and
are never copied into the tree.

A project can carry its own `.niki/` directory for skills and hooks, so a
repository can ship its Niki setup with it.

## One environment namespace

Every `NIKI_*` variable means the same thing on Linux, macOS and Windows.
Provider keys keep their vendor names (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`,
and so on) so an existing shell profile works unchanged.

| Variable | Effect |
| --- | --- |
| `NIKI_HOME` | Moves the configuration, skills, hooks and session store. |
| `NIKI_ASCII` | Draws the full-screen app with ASCII characters only. |
| `NIKI_REDUCED_MOTION` | Stops the spinner. |
| `NIKI_MCP_STARTUP_TIMEOUT_SECS` | Startup budget for each MCP server, in seconds. |

## One release pipeline

The release build is `cargo build --release -p niki --bin niki`. The release
publishes platform archives named `niki-<version>-<platform>-<arch>.tar.gz`, a
Windows `.zip`, and a `SHA256SUMS` file that covers every archive. The installer
verifies the checksum before it places the binary on `PATH`.

Linux x86_64 has a prebuilt archive. Windows has an archive that has not been
exercised on a Windows machine, so treat it as untested. macOS builds from
source; there is no prebuilt macOS archive.

## Upgrades

A newer Niki reads the same `config.toml`, the same `hooks.json`, the same skill
directories and the same `sessions.db`. Unknown configuration keys are ignored,
so a file written for a later release still loads. The configuration also
accepts the `[models.providers.<name>]` spelling alongside `[providers.<name>]`,
so an older layout keeps working. `[providers.<name>]` wins when both name the
same provider.

Shell commands that start a turn (`niki`, `niki -n`) keep their meaning across
releases. A script that runs `niki -n "…"` before an upgrade still runs after
one.
