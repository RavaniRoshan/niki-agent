# Your first turn

## Start a session

Open a terminal in the repository you want to work on and run:

```bash
niki
```

The session starts in the current directory. The first launch asks your name and
leaves you at a working composer — no account and no API key are required to
look around.

Type a request and press <kbd>Enter</kbd>. Niki reads the repository, proposes a
tool call, and asks before it changes anything in Manual mode (the default).

## Add a model

Anthropic, OpenAI, and Google Gemini work immediately. Set one key in the
environment you launch from:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
export OPENAI_API_KEY=sk-...
export GOOGLE_API_KEY=...
```

Or store a key from inside the session: `/auth` shows which providers are
configured, accepts a pasted key, and links to the provider's key page. A key
pasted there is written to `~/.niki/.state/auth.json` with mode `0600`; an
environment variable is never rewritten.

`/model` picks a provider and model. Eighteen more provider integrations are one
`/install` away — `/install ollama`, `/install openrouter`, and so on. The full
table, the configuration keys, and the recipe for any OpenAI-compatible endpoint
are in [Providers](/providers/).

## The commands you will use first

Slash commands are typed into the composer. `/help` lists every one.

| Command | What it does |
| --- | --- |
| `/model` | Switch models or edit model settings. |
| `/auth` | Connect and manage provider and service credentials. |
| `/theme` | Change the colour theme. |
| `/install` | Install an optional integration. |
| `/manual`, `/auto`, `/yolo` | Switch approval mode. |
| `/docs` | Open the documentation. |
| `/help` | Show help and available commands. |

The options that matter before the first turn are also on the command line:

| Command | What it does |
| --- | --- |
| `niki --version` | Print the version. Answers before the UI loads. |
| `niki --help` | List every option and subcommand. |
| `niki -r` | Resume the most recent thread (`-r ID` for a specific one). |
| `niki threads list` | List saved threads. |
| `niki doctor` | Print install diagnostics. |
| `niki config path` | Print the configuration file path. |

## Keys worth knowing

| Key | Action |
| --- | --- |
| <kbd>Enter</kbd> | Submit the prompt. |
| <kbd>Ctrl</kbd>+<kbd>J</kbd> | Insert a newline (also <kbd>Shift</kbd>+<kbd>Enter</kbd>). |
| <kbd>Shift</kbd>+<kbd>Tab</kbd> | Cycle the approval mode. |
| <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd> | Open the command palette. |
| <kbd>Esc</kbd> | Interrupt the turn. |
| <kbd>?</kbd> | Show help. |
| <kbd>Ctrl</kbd>+<kbd>C</kbd> | Quit, or interrupt if a turn is running. |

Every binding is in the [keyboard reference](/keyboard), which is generated from
the live binding registry.

## Headless mode

`-n/--non-interactive` runs one task and exits, with the answer on stdout:

```bash
niki -n "summarize this repo"
```

Headless runs have no human approvals, can change files and access the network,
and have the shell turned off unless a shell allow-list is configured. Useful
flags:

```bash
niki -n "list the TODO markers" -q            # clean output for piping
niki -n "fix the failing test" -S recommended # a safe set of shell commands
niki -n "summarize" --timeout 120             # wall clock; exit 124 on expiry
cat prompt.txt | niki --stdin -q              # explicit stdin
```

## Where your session lives

Everything Niki writes lives under `~/.niki`:

```text
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

`NIKI_HOME` moves the whole tree. A repository can also carry its own `.niki/`
directory for skills, agents, hooks, and MCP configuration, so a project can ship
its Niki setup with it.

Next: [Permissions and sandboxing](/permissions) explains what the three
approval modes let through.
