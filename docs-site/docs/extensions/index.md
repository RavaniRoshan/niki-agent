# Extensions and hooks

<!-- Summarises libs/code/HOOKS.md, libs/code/EXTENSIONS.md, and the plugin,
     skill, and MCP surfaces documented in the repository. Those files stay the
     source of truth. -->

Niki can be extended without changing the package. There are five surfaces, and
they have different trust rules.

| Surface | Where it lives | What it adds |
| --- | --- | --- |
| Instructions | `NIKI.md` or `AGENTS.md` in a project, per-agent profiles | Project and agent context the model reads. |
| Skills | `~/.niki/agent/skills/`, a project's `.niki/` | Named, reusable prompts you invoke by name. |
| Plugins | `~/.niki/plugins/` | A versioned bundle that can carry skills, hooks, and Python extensions. |
| Hooks | `~/.niki/hooks.json`, `.niki/hooks.json`, a plugin's `hooks/hooks.json` | Shell commands on agent lifecycle events. |
| Python extensions | a plugin manifest, `~/.niki/extensions/`, `.niki/extensions/` | Middleware, tools, and virtual storage routes inside the agent server. |

MCP servers are a sixth: `~/.niki/.mcp.json` lists them, `/mcp` manages them, and
`niki mcp login` authenticates one. They run as you, with your filesystem and
network access.

## Skills

A skill is a named prompt the agent can run on request. Manage them with the CLI:

```bash
niki skills list
niki skills create
niki skills info <name>
niki skills delete <name>
```

User skills live in `~/.niki/agent/skills/`. A project can carry its own in
`.niki/`. Start a session with a skill already invoked with `-s/--skill NAME`, for
example `niki --skill code-review -m 'review this patch'`. `/skill-creator` walks
through writing one inside a session.

## Plugins

`/plugins` manages installed plugins. A plugin is identified as
`name@marketplace`, loads from a versioned cache, and may contribute:

- a `plugin.json` manifest,
- `hooks/hooks.json`, or a `hooks` path or inline object in the manifest,
- Python extensions declared under `extensions` in the manifest.

Installing and enabling a plugin is the consent gate for its hooks and
extensions. That is separate from project trust, which governs the `.niki/`
directory of the repository you are working in. Newly enabled plugin hooks take
effect after `/reload`; backend changes from Python extensions need `/restart`.

## Hooks

Hooks are user-configured shell commands that run at lifecycle events such as
`SessionStart`, `PreToolUse`, and `Stop`. Each handler receives a JSON event on
stdin and can influence the session through its exit code and stdout.

[Hooks](/extensions/hooks) covers the configuration locations, every event and
matcher, the handler shape, and how output changes behaviour.

## Python extensions

Python extensions customize the agent server: middleware, tools, and virtual
storage routes. They are experimental, load only when `NIKI_EXPERIMENTAL=1` is
set before starting `niki`, and are meant to ship inside a plugin.

[Python extensions](/extensions/python) covers the manifest, the registration
API, storage routes, discovery order, and the configuration.

## Trust rules at a glance

Project-scoped code is read from the repository you opened, so each surface has
its own prompt and its own flag for headless runs:

| Surface | Headless opt-in |
| --- | --- |
| `.niki/hooks.json` | `--trust-project-hooks` |
| `.niki/extensions` | `--trust-project-extensions` |
| Project MCP configuration | `--trust-project-mcp` |

Interactive launches ask before loading any of them. Only trust projects you
control, and read a hook, extension, or plugin before you enable it.
