# How Niki Agent fits together

Niki Agent is a Rust workspace in `rust/`. One binary, `niki`, links the turn
loop, the tool registry, the approval policy, the provider adapters, the MCP
client, the session store and the terminal UI. Nothing else has to be installed
to run it.

## The shape of a turn

```
niki                      niki -n "prompt"          niki --acp
  -> load config, provider, workspace, tools, policy, MCP, hooks, skills
  -> Agent::run (niki-agent)
       UserPromptSubmit hook
       provider request  <-- model streams text and tool calls
       tool call
         PreToolUse hook
         policy gate (mode, allow-list, path boundary, MCP approval)
         tool runs
         PostToolUse hook
       tool result back to the model
     until the model stops calling tools, the step budget runs out, or a cancel
  -> AgentEvent stream -> full-screen app, stdout, or ACP notifications
```

A turn is a loop of model requests and tool calls. The provider streams events;
the agent assembles each tool call from the fragments the wire format splits it
into, asks the policy, runs the tool, and sends the result back as a message.
The loop ends when the model returns no tool calls, when `--max-steps` is
reached, or when the person cancels.

## Crates

| Crate | Responsibility |
| --- | --- |
| `niki` | The binary: `niki` opens the full-screen app, `niki -n` runs one prompt headlessly, `niki --acp` serves an editor. |
| `niki-config` | `config.toml`: provider blocks, `[models].default`, `[ui].theme`, `[mcp_servers.<name>]`, and `NIKI_HOME`. |
| `niki-provider` | One streaming turn against an OpenAI Chat Completions or Anthropic Messages endpoint, including retries and usage. |
| `niki-agent` | The turn loop, the event callback, the approval callback, the step budget, hooks, cancellation, token accounting and compaction. |
| `niki-acp` | The Agent Client Protocol server over stdin/stdout: sessions, notifications, and the permission round-trip to an editor. |
| `niki-tools` | `read_file`, `write_file`, `edit_file`, `list_dir`, `shell`, `grep`, `glob`, all rooted at the workspace. |
| `niki-subagents` | The `task` tool: one child agent per call with a fresh history, a shared cancel handle and a bounded step budget. |
| `niki-policy` | Manual/Auto/YOLO, the shell allow-list, and the workspace path boundary. |
| `niki-workspace` | Root detection, ignore rules, and the file index. |
| `niki-session` | The SQLite transcript store: create, append, list, load, and the per-session summary. |
| `niki-mcp` | MCP handshake, `tools/list`, `tools/call` over stdio and streamable HTTP; tool bridges. |
| `niki-hooks` | `hooks.json` loading, matcher selection, command handlers, and blocking verdicts. |
| `niki-skills` | Skill discovery from the project, the user directory and the built-ins, and the system-prompt section. |
| `niki-commands` | The slash command grammar the composer and help overlay read, and the roadmap of commands not yet implemented. |
| `niki-render` | The transcript view model, the deterministic frame, the Markdown line renderer, and the unified-diff renderer with line numbers. |
| `niki-tui` | The keymap, reader mode, and view state. |
| `niki-tui-live` | The full-screen app: composer, transcript, status bar, worker, and session persistence. |
| `niki-tui-modals` | The approval prompt, model picker, session browser, and help overlay. |
| `niki-tui-themes` | Four palettes, the contrast gate, and glyph and motion modes. |
| `niki-proto`, `niki-engine-client` | The engine wire vocabulary and client. They are not on the product turn path. |

## The agent loop

`niki-agent` owns the conversation and the loop. It has no opinion about
transport: any provider streams the events it already normalises, and any
registered tool is dispatched through the same gate.

- A tool failure and a policy refusal are **tool results**, not agent errors, so
  the model can read them and react.
- A provider failure aborts the run, after `niki-provider` has already retried
  the request twice when the failure was transient (429, 5xx, connection loss)
  and the stream had not started.
- The transcript is append-only. A cancelled turn keeps what it already
  produced.
- `Decision::Ask` goes to the approval callback the host installed. A headless
  run installs none, so the request is reported to the model as a refusal
  instead of silently running.

  Three kinds of action reach that callback. `shell` is decided per command
  against the allow-list. `write_file` and `edit_file` are gated as tools, and
  `Auto` runs one unattended when its target resolves inside the workspace;
  `Manual` asks, and a path that leaves the workspace fails resolution and
  takes the asking path. A mutating MCP tool is always gated, because a server
  may reach the network whatever it says it touches.

### Delegation and context

A `task` call runs a second agent inside the first. The child gets a registry
derived from the session's with `task` removed, so it cannot delegate again; it
starts with an empty history, shares the parent's cancel handle, and stops after
20 model turns unless `NIKI_SUBAGENT_MAX_STEPS` says otherwise. The parent
receives the child's answer and a one-line summary, not its exploration.

`Agent` also counts what a session spends and weighs: `context_report()` gives
the message count, an estimated token figure for the conversation, and the last
reported input and output tokens; `context_audit()` splits the bill into the
system prompt, the tool schemas and the messages; `compact(provider, keep_last)`
folds the turns older than the last `keep_last` into one summary written by the
model, and leaves the history untouched if the provider fails or answers
nothing. The session store keeps every message either way, so `/resume`
restores the transcript as it was written, not as it was compacted.

## Entry points

| Surface | Owns |
| --- | --- |
| Full-screen (`niki`) | Input, approvals, the transcript, the `Ctrl+T` overlay that shows the whole conversation, and presentation. |
| Headless (`niki -n`) | One prompt and exit; the answer goes to stdout, tool activity to stderr. `--output-format json` prints one document; `--output-format jsonl` streams one event per line (`start`, `text`, `tool_call`, `tool_result`, `usage`, `end`). |
| Editor (`niki --acp`) | The Agent Client Protocol on stdin/stdout; the editor answers permission requests. |

All three entry points build the same agent from the same configuration. The
full-screen app runs the turn on a worker thread with its own runtime, talks to
it over a channel, and answers approvals over a one-shot reply. Dropping that
reply unblocks the turn with a refusal, which is what Esc and Ctrl-C rely on.
The app offers the model picker, themes, sessions and slash commands; the
headless path prints the answer and a one-line summary; the ACP server turns the
same events into `session/update` notifications and turns a gated action into a
`session/request_permission` request.

## Extensions

Three surfaces extend a session without changing the binary:

- **MCP servers** from `[mcp_servers.<name>]`. Their tools join the built-in
  registry under `mcp__<server>__<tool>`.
- **Hooks** from `.niki/hooks.json` and `~/.niki/hooks.json`. They run shell
  commands around a prompt and a tool call, and may block either.
- **Skills** from `.niki/skills/<name>/SKILL.md`, `~/.niki/skills/<name>/SKILL.md`
  and two built-ins. Their names and descriptions are injected into the system
  prompt; the model reads the body only when a skill applies. A project skill is
  read only while it resolves inside the workspace; the user skills directory is
  the user's own and is read as given.

Instruction files are the fourth: `AGENTS.md` and `.niki/AGENTS.md` from the
repository root down to the working directory, with `$NIKI_HOME/AGENTS.md` (or
the platform config home) first. Each file is truncated on a line boundary
under a 32 KiB budget shared by the chain, and a link that resolves outside the
workspace is skipped.

## State

Everything Niki writes lives under `NIKI_HOME` when set, otherwise `~/.niki`:

```
~/.niki/
  config.toml            # providers, default model, theme, MCP servers
  hooks.json             # user hooks
  skills/                # user skills
  sessions.db            # SQLite transcript store
  history.jsonl          # prompts accepted in the full-screen app
```

A project may also carry `.niki/hooks.json` and `.niki/skills/`. The workspace
root is the current directory unless `--root` names another one.
