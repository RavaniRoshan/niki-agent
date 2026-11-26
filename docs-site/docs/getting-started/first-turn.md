# Your first turn

## Configure a model

The full-screen app needs a model before it starts. A built-in provider needs
only its endpoint (already known) and a key in the environment; name the default
model in `~/.niki/config.toml`:

```toml
[models]
default = "anthropic:my-model"
```

```bash
export ANTHROPIC_API_KEY=sk-ant-...
```

Replace the model ID with one your provider serves. The built-in providers and
their key variables are listed in [Providers](/providers/); any
OpenAI-compatible gateway is a `[providers.<name>]` block.

Without a default model and without `--model`, Niki exits with:

```text
no model selected: set [models].default in ~/.niki/config.toml or pass --model provider:model
```

## Start a session

Open a terminal in the repository you want to work on and run:

```bash
niki
```

The session starts in the current directory, or in `--root <PATH>`. Type a
request and press <kbd>Enter</kbd>. Niki reads the repository, proposes a tool
call, and asks before running a shell command that is not on the allow-list.
Built-in file writes run without asking in this build; see
[Permissions](/permissions) for the exact gate.

`niki` with no `-n` opens the full-screen app and must have a terminal. When
stdout is not a terminal, use `-n`:

```text
the full-screen app needs a terminal; pass -n <PROMPT> to run one prompt headlessly
```

## Flags

The complete option list:

| Flag | What it does |
| --- | --- |
| `-n`, `--prompt <TEXT>` | Run one prompt headlessly and exit. Without it, the app opens. |
| `--acp` | Serve an editor over stdin/stdout. Cannot be combined with `-n`. |
| `--model <PROVIDER:MODEL>` | Select the model. Defaults to `[models].default`. |
| `--config <PATH>` | Config file. Defaults to `$NIKI_HOME/config.toml`, else `~/.niki/config.toml`. |
| `--root <PATH>` | Workspace root. Defaults to the current directory. |
| `--mode <MODE>` | `manual`, `auto`, or `yolo`. Default `manual`. |
| `--allow <SPEC>` | Comma-separated shell allow-list, e.g. `"git*,cargo*"`. |
| `--max-steps <N>` | Model turns allowed. Default 40. |
| `--system <TEXT>` | Replace the system prompt. |
| `--quiet` | Do not report tool calls or the summary on stderr. |
| `--version`, `-V` | Print the version line and exit. |
| `--help`, `-h` | Print the option list and exit. |

`-q` is accepted as an alias for `--quiet`. An unknown option, a missing value,
or a positional argument is an error; the prompt is always passed with `-n`.
An unknown `--mode` is a warning, and the run continues in `manual`.

## Exit status

| Code | Meaning |
| --- | --- |
| `0` | The turn finished. A refused tool is a result, not a failure. |
| `1` | A configuration, provider, or workspace failure. |
| `2` | A usage error: `--acp` combined with `-n`. |
| `130` | Cancelled with <kbd>Ctrl</kbd>+<kbd>C</kbd>. |

## Slash commands

Type these in the composer. `/help` lists the same set.

| Command | What it does |
| --- | --- |
| `/clear` | Start a fresh session and empty the view. |
| `/compact` | Fold older rows out of the view; the session store keeps them. |
| `/help` | The key and command list. |
| `/model` | Choose a model, or name one with `/model provider:model`. |
| `/quit` | Leave the app. |
| `/resume` | List saved sessions, or resume one by id or index. |
| `/skills` | List project and user skills. |
| `/theme` | Choose a palette, or cycle to the next one. |
| `/tools` | List the registered tools. |
| `/version` | Show the version. |

`/compact` only hides older rows; this build has no summarising model.

## Keys worth knowing

| Key | Action |
| --- | --- |
| <kbd>Enter</kbd> | Send the prompt. |
| <kbd>Alt</kbd>+<kbd>Enter</kbd> | Insert a newline. |
| <kbd>Shift</kbd>+<kbd>Tab</kbd> | Cycle manual, auto and yolo. |
| <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd> | Start a slash command in the composer. |
| <kbd>Esc</kbd> | Clear the draft, then stop the running turn. |
| <kbd>?</kbd> | Show help. |
| <kbd>y</kbd> / <kbd>n</kbd> | Allow or refuse the approval prompt. |
| <kbd>Ctrl</kbd>+<kbd>C</kbd> | Stop the running turn; leave when nothing is running. |

Every binding is in the [keyboard reference](/keyboard).

## Themes and terminal settings

Four palettes are built in: `niki`, `high-contrast`, `colorblind`, and `mono`.
`/theme` cycles them, `/theme <name>` selects one, and `[ui].theme` sets the
startup palette.

```bash
NIKI_ASCII=1 niki          # draw the app with ASCII characters only
NIKI_REDUCED_MOTION=1 niki # stop the spinner without changing the layout
```

## Headless mode

`-n/--prompt` runs one prompt and exits. The answer goes to stdout and tool
activity goes to stderr, so a pipe captures the answer and nothing else:

```bash
niki -n "summarize this repository"
niki -n "list the TODO markers" --quiet > todos.txt
niki -n "fix the failing test" --root ../other-repo --max-steps 60
```

A headless run has nobody to ask, so a shell command that the policy would ask
about is reported to the model as a refusal. Put the command on `--allow`, or
run the same prompt in the app, when you want it to proceed.

## Editor protocol

`--acp` serves the Agent Client Protocol on stdin/stdout, so an editor can drive
the same agent, provider and workspace. The editor answers a gated action through
`session/request_permission`. Only protocol frames may reach stdout while this
runs; warnings go to stderr. `--acp` cannot be combined with `-n`:

```text
--acp and -n cannot be combined; --acp serves an editor on stdin/stdout and -n runs one prompt
```

This build emits text and tool-call updates but no reasoning chunks, refuses
image and audio prompt blocks, ignores `mcpServers` and `additionalDirectories`
in the protocol, and `session/load` replays the transcript without seeding the
model's context. Tools still come from `config.toml` at startup.

## Where your session lives

```text
~/.niki/
  config.toml            # providers, default model, theme, MCP servers
  hooks.json             # user lifecycle hooks
  skills/                # user skills
  sessions.db            # SQLite transcript store
```

`NIKI_HOME` moves the whole tree. The full-screen app opens `sessions.db` and
writes every completed turn; `/resume` lists the saved sessions on the next
launch. A project can also carry `.niki/hooks.json` and `.niki/skills/`.

Next: [Permissions](/permissions) explains what the three approval modes let
through.
