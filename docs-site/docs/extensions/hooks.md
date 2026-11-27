# Hooks

Hooks are shell commands that run at lifecycle events. Each matching handler
receives one JSON object on stdin and may add context, show a notice, or block
the event.

::: danger
Hook commands run on your machine with your user privileges. Treat every
`hooks.json` entry as code you trust — especially a project file checked into a
repository. Loading `.niki/hooks.json` is not behind a trust prompt in this
build.
:::

## Locations and precedence

| Scope | Path |
| --- | --- |
| Project | `{workspace_root}/.niki/hooks.json` |
| User | `{config_home}/hooks.json` — normally `~/.niki/hooks.json` |

The project file wins for an event it defines; the user file supplies the events
it does not. A missing file is normal. A file that cannot be read, is not valid
JSON, or has an unusable entry produces a warning and the session continues. A
headless run prints the warning on stderr; the full-screen app adds one warning
row to the transcript before the first frame.

Only `UserPromptSubmit`, `PreToolUse` and `PostToolUse` are implemented. Any
other event name in the file is ignored with a warning.

## Shape

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

A bare map of event name to matcher groups is also accepted, without the outer
`"hooks"` key.

A handler needs a `command` string. `type`, when present, must be `"command"`.
`timeout` is a positive number of seconds. `async: true` is rejected, and
unsupported fields are ignored.

Every matching handler runs, in configuration order, as a `/bin/sh -c` child with
the workspace root as its working directory. Handlers are synchronous: the turn
waits for them.

## Events and matchers

| Event | Matcher target | Fires when |
| --- | --- | --- |
| `UserPromptSubmit` | _(none)_ | The prompt is about to reach the model. |
| `PreToolUse` | `tool_name` | A tool call is about to run. |
| `PostToolUse` | `tool_name` | A tool call returned. |

A group may omit `matcher`, set `""` or `"*"` to match everything. A matcher
made only of letters, digits, `_`, whitespace, `-`, `,` and `|` is a list of
exact names; anything else is an unanchored regular expression. `UserPromptSubmit`
has no matcher target, so a non-empty matcher there is ignored with a warning.

Matchers use the wire tool names, so an existing `hooks.json` keeps selecting
the same tool:

| Built-in tool | Wire name |
| --- | --- |
| `shell` | `Bash` |
| `read_file` | `Read` |
| `write_file` | `Write` |
| `edit_file` | `Edit` |
| `list_dir` | `LS` |
| `grep` | `Grep` |
| `glob` | `Glob` |
| `mcp__<server>__<tool>` | unchanged |

The tool arguments are translated the same way: `path` becomes `file_path`,
`old`/`new` become `old_string`/`new_string`, and `shell` keeps `command` and
`timeout`.

## Payload

Every handler receives one JSON object:

| Field | Present on |
| --- | --- |
| `hook_event_name` | every event |
| `cwd` | every event |
| `session_id` | when the host has one |
| `tool_name`, `tool_input`, `tool_use_id` | `PreToolUse`, `PostToolUse` |
| `tool_response`, `duration_ms` | `PostToolUse` |
| `prompt` | `UserPromptSubmit` |

## Verdicts

A handler influences the turn through its exit code and stdout:

- **Exit `2`** blocks the event. The trimmed stderr is the reason.
- **JSON `{"decision":"block","reason":"…"}`** blocks the event.
- **`{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"…"}}`**
  also blocks `PreToolUse`.
- **`{"continue": false, "stopReason": "…"}`** stops the event with that reason.
- **`additionalContext`** adds text to the tool result, or beside the prompt.
- **`systemMessage`** is shown to the person as a notice. `suppressOutput: true`
  hides it.

A blocked `PreToolUse` never reaches the tool; the tool result is
`<wire name> blocked by hook: <reason>` (for example `Bash blocked by hook: …`),
marked as an error, so the model can read it and try another way. A blocked
`UserPromptSubmit` stops the prompt before any model request. A blocked
`PostToolUse` is feedback appended to the result, because the tool has already
run.

`permissionDecision` values other than `deny` (`allow`, `ask`, `defer`),
`updatedInput`, `updatedToolOutput`, `updatedMCPToolOutput` and
`suppressOriginalPrompt` are not implemented; the runner reports them as
warnings rather than acting on them.

A handler that exits non-zero for another reason, is terminated by a signal,
times out, or writes output this build cannot read is a warning, and the turn
continues. Plain text on `UserPromptSubmit` is the exception: it becomes
additional context. The default timeout is 600 seconds; `UserPromptSubmit`
defaults to 30 seconds. A handler may set its own `timeout`.

## Example: block a destructive command

The matcher selects the shell tool by its wire name. The handler searches the
payload for the command and exits `2` with a reason:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "if grep -q 'rm -rf /' ; then echo 'refusing a root delete' >&2; exit 2; fi"
          }
        ]
      }
    ]
  }
}
```

A refusal is visible to the model as a tool result; it is not an agent failure,
and the turn can continue.

## Example: add context to every prompt

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "printf 'The repository is read-only today: do not edit files.'"
          }
        ]
      }
    ]
  }
}
```

On `UserPromptSubmit`, plain stdout that is not JSON is additional context.
