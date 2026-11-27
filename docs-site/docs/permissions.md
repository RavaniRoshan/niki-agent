# Permissions

Niki runs shell commands and reads and writes files with your user's
permissions. The approval mode, the shell allow-list, the workspace path
boundary and the approval callback decide how much of that it may do without
asking.

::: warning
There is no sandbox in this build, and no classifier. Auto asks exactly like
Manual. A built-in file write is not gated yet. Run Niki in a container or under
a dedicated user account when you need a hard boundary; see
[Security](/security).
:::

## The three modes

<kbd>Shift</kbd>+<kbd>Tab</kbd> cycles **Manual → Auto → YOLO**, and the status
bar names the active mode. `--mode manual|auto|yolo` selects the startup mode.

| Mode | Shell commands that run without asking |
| --- | --- |
| Manual | Only commands on the allow-list. Everything else asks. |
| Auto | The same as Manual. There is no classifier in this build, so Auto only changes the label. |
| YOLO | Every command the policy does not deny. Nothing asks. |

Manual is the default. An unknown `--mode` value prints a warning and starts in
Manual.

## How a shell command is decided

For one shell call, the policy answers in this order:

1. An empty command is refused. No mode can upgrade that refusal.
2. In YOLO, the command runs.
3. If the command is on the allow-list, it runs.
4. Otherwise the call asks: the full-screen app shows the approval prompt, an
   ACP session sends `session/request_permission` to the editor, and a headless
   run has nobody to ask and reports a refusal to the model.

The prompt offers three answers. `y` or `a` runs the call once. `s` allows it
for the rest of the session: a shell rule records the program and the exact
line, and a later command that chains, redirects, or substitutes another
command still asks — approving `git status` must not silently run
`git status; rm -rf ~`. `d`, `n`, or `Esc` refuses. `/clear` and a new session
drop every session rule, and nothing is written to `config.toml`.

The model sees an approval outcome as a tool result, not as an agent failure,
so it can adapt to a refusal.

### Allow-list syntax

`--allow` takes a comma- or newline-separated list:

- An entry ending in `*` matches any command that starts with the text before
  the star. `git*` matches `git status` and `git`.
- Any other entry matches the whole command exactly. `ls` matches `ls` but not
  `ls -la`.
- The entry `all` (any case) disables the check. An empty list permits nothing.

```bash
niki --allow 'git*,cargo test*'      # prefixes
niki -n "run the tests" --allow 'cargo*'
```

`s` on an approval prompt is the session-scoped answer. It never rewrites
`config.toml`; a command you want on every run belongs in `--allow` or the
`[shell]` section.

A command that is refused on the command line is still a valid turn result:
`niki -n` exits `0`.

## Built-in tools

| Tool | Gate |
| --- | --- |
| `read_file`, `list_dir`, `grep`, `glob` | No approval prompt. Reads only. |
| `write_file`, `edit_file` | Ask in Manual and Auto. Auto runs one unattended only when the target resolves inside the workspace; a target outside always asks. |
| `shell` | The mode and the allow-list above. |

A write prompt names the absolute path it would touch and says whether that path
resolves inside the workspace, so the answer is made against the real target
rather than the argument the model typed.

## The workspace path boundary

Every tool path is joined to the workspace root and resolved through its nearest
existing ancestor. Symlinks are followed before the check. A result that leaves
the root is refused, not clamped:

- `..` that climbs above the root is refused.
- A symlink that points outside the root is refused, for an existing file and
  for a new one through that directory.
- A dangling symlink is refused rather than treated as a path to create.
- An absolute path that resolves outside the root is refused.

The `shell` tool also resolves its `cwd` against the root. The command itself
runs with `/bin/sh -c` and is not sandboxed, so it can reach anything your user
can reach. The path boundary governs tool arguments, not the command's own
effects.

## MCP tool approval

An MCP tool joins the registry as `mcp__<server>__<tool>`. Its approval
requirement comes from the server's tool annotation:

- A tool whose server declares `readOnlyHint = true` (and not a contradictory
  `destructiveHint`) runs without asking.
- Every other MCP tool is gated. Manual and Auto both ask; YOLO runs it.
- `s` on the prompt records the tool name for the session. It is separate from
  the shell allow-list.
- A headless run has no approver, so a gated MCP call is reported to the model as
  a refusal; the full-screen app and an ACP session ask the person or editor.

An MCP server runs as you and may reach the network and filesystem. The approval
covers the call, not the server's behaviour after it.

## Hooks can block

Hooks run around the prompt and around a tool call, in every mode:

- `UserPromptSubmit` runs before the prompt reaches the model. A blocking
  handler stops the prompt.
- `PreToolUse` runs before a tool. A blocking handler stops the call; the tool
  result is `<wire name> blocked by hook: <reason>`, so the model can read it and
  try another way.
- `PostToolUse` runs after a tool. A block is feedback appended to the result,
  because the tool has already run.

Hooks are shell commands, so a hook can do anything your user can do. Read a
project's `.niki/hooks.json` before trusting the repository. See
[Hooks](/extensions/hooks).

## Hard boundaries

Because this build has no sandbox, the reliable boundaries are outside the
process:

- Run Niki in a container or under a user that can only write the project.
- Keep `--allow` narrow; never use `all` outside a throwaway environment.
- Stay in Manual for untrusted work.
- Read hooks and skills before running Niki in a repository you did not write.

## What to read next

- [Security](/security) — trust boundaries, gaps, and the hardening checklist.
- [Providers](/providers/) — endpoints and key variables.
- [Hooks](/extensions/hooks) — the `hooks.json` contract.
