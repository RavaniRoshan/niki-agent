# Extensions

Niki can be extended without changing the binary. There are three surfaces, and
they carry different trust.

| Surface | Where it lives | What it adds |
| --- | --- | --- |
| Skills | `.niki/skills/<name>/SKILL.md`, `~/.niki/skills/<name>/SKILL.md`, two built-ins | Named instruction packs the model reads when one applies. |
| Hooks | `.niki/hooks.json`, `~/.niki/hooks.json` | Shell commands around a prompt or a tool call that can add context or block. |
| MCP servers | `[mcp_servers.<name>]` in `config.toml` | Tools from an external process or HTTP endpoint, registered beside the built-ins. |

There is no plugin loader and no extension API beyond these three surfaces in
this build. A skill is instruction text; hooks and MCP servers are code that runs
as you.

## Skills

A skill is a directory holding a `SKILL.md`: YAML front matter with a `name` and
a `description`, then instructions for a recurring job. Discovery takes the first
match for a name in this order:

1. `<workspace>/.niki/skills/<name>/SKILL.md`
2. `~/.niki/skills/<name>/SKILL.md` (or `$NIKI_HOME/skills`)
3. the built-in `commit` and `review` skills, compiled into the binary

The model sees each skill's name, description and, for a project skill, the path
to read. It reads the body only when the skill applies, so a long skill costs
nothing on a turn that does not use it. Built-in bodies are short and are
included directly. A user skill under `~/.niki/skills` is outside the workspace
root, and `read_file` refuses a path there in this build; put a skill the model
must read in `.niki/skills/`. `/skills` lists what discovery found.

[Skills](/extensions/skills) covers the format, precedence, warnings, and the
two built-ins.

## Hooks

A `hooks.json` maps an event to matcher groups and command handlers. This build
runs `UserPromptSubmit`, `PreToolUse` and `PostToolUse`; other event names are
ignored with a warning. A handler receives one JSON payload on stdin and can add
context, show a notice, or block the event.

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          { "type": "command", "command": "check-shell.sh", "timeout": 60 }
        ]
      }
    ]
  }
}
```

[Hooks](/extensions/hooks) covers locations, matchers, payloads, verdicts and
timeouts.

## MCP servers

MCP servers are configured in `config.toml`. Two transports are supported:
stdio, and streamable HTTP.

```toml
# ~/.niki/config.toml

[mcp_servers.fs]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
env = { LOG_LEVEL = "debug" }
cwd = "/tmp/work"

[mcp_servers.docs]
url = "https://example.com/mcp"
headers = { Authorization = "Bearer token" }
```

An entry without `type` infers the transport: `url` selects HTTP, `command`
selects stdio. `type = "stdio"` and `type = "http"` (or `streamable_http`) can
state it explicitly. A legacy `type = "sse"` long-lived GET stream is rejected at
load with a message naming the server. `${VAR}` interpolation in `command`,
`args`, `env`, `url` and `headers` is not implemented; write the values
literally.

At startup both entry points connect every configured server, concurrently, and
register each tool the server advertises as `mcp__<server>__<tool>`. A server
that fails to start does not stop the session: it is a warning line and the
remaining servers' tools are kept. The handshake budget is 10 seconds per
server; set `NIKI_MCP_STARTUP_TIMEOUT_SECS` to change it for a slow server.

A tool the server does not declare read-only is approval-gated; a read-only tool
runs without asking. A headless run reports a gated call to the model as a
refusal. `/tools` lists the registry, including `mcp__` names.

MCP servers run as you and may reach the network and filesystem. The approval
covers one call, not the server's behaviour after it. See
[Permissions](/permissions).

## Trust at a glance

| Surface | Read from a repository? | Runs code? |
| --- | --- | --- |
| Skills | `.niki/skills/` | No; it is instruction text. |
| Hooks | `.niki/hooks.json` | Yes; each handler is a shell command. |
| MCP servers | `config.toml` (user configuration) | Yes; the server process or HTTP endpoint runs as you. |

Read a hook or an MCP server configuration before you trust it. Project hooks
are not behind a trust prompt in this build.
