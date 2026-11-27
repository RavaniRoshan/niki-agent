# Sessions

Niki keeps its conversations in one SQLite database. The full-screen app, a
headless run with `-n`, and an editor session served by `--acp` all read and
write that file; `niki sessions list` reads it without starting any of them.

## Where the store lives

The database is `sessions.db` in the configuration directory: `$NIKI_HOME` when
it is set, otherwise `~/.niki`.

```text
$NIKI_HOME/sessions.db
~/.niki/sessions.db      # NIKI_HOME unset
```

A run creates the directory and the database. `niki sessions list` and
`niki doctor` read the database only when it already exists, so neither of them
creates one. On Unix the file is `0600` whenever a surface opens it, and the app
and a headless run create the directory `0700`: a transcript holds prompts,
answers and tool output, and another account on the machine cannot read it.

Two tables carry what you see and resume:

| Table | Row |
| --- | --- |
| `sessions` | `id`, `created_at`, `title`, `cwd` |
| `messages` | autoincrement `id`, `session_id`, `role`, `content`, `created_at` |

Timestamps are Unix seconds. Message order is the autoincrement `id`, so a
transcript replays in the order it was written. A session id is opaque: 32 hex
digits and an attempt suffix, `000000000000000018ebde128ab2bf5e-00`. Pass a
whole id back to `--resume` or `/resume`; do not parse one.

## What is saved

Every turn that completes is appended to the session it ran in. A row is one
stored message, and the `role` decides how it reads:

| Role | Written by | Content |
| --- | --- | --- |
| `user` | every surface | The prompt, verbatim. |
| `assistant` | every surface | The answer text. |
| `tool_call` | the app and a headless run | Tool name on the first line, arguments below. |
| `tool_result` | the app and a headless run | Tool name on the first line, output below. |
| `tool_error` | the app | A tool result that failed. |
| `error` | the app | A turn that ended with an error. |

A headless run writes in this order after the turn returns without an error:
the prompt, the tool rows, then the answer. The full-screen app appends the rows
of a turn when the turn ends, and a failed or cancelled turn is kept as far as
it got.

`title` starts empty. Only `/rename` and the headless `-n "/rename …"` form
write it. Until then `niki sessions list` shows `(untitled)`, and `/threads`
and an export fall back to the first prompt.

If the store cannot be opened, Niki says so and keeps running. A headless run
prints `niki: warning: sessions are not saved: …` on stderr, and the app shows
the same warning in the transcript. A run that cannot save cannot resume either:
`--resume` and `--continue` need the store and fail without it.

## Continuing from the shell

Three flags select a saved session for a headless run:

| Flag | Session |
| --- | --- |
| `--continue` | The most recently created session for this workspace. |
| `--resume last` | The same session `--continue` picks. |
| `--resume <ID>` | The session with exactly that id. |

The workspace is `--root`, or the current directory. The stored `cwd` text is
compared as it was written, so run from the same directory the session was
created in.

A resumed run sends the saved `user` and `assistant` rows again, oldest first,
before the new prompt. Tool rows stay in the store and are not resent, because a
stored row carries no tool-call id for the model to answer.

```bash
niki sessions list                                   # find the id first
niki --continue -n "now run the tests"
niki --resume 000000000000000018ebde128ab2bf5e-00 -n "and update the changelog"
```

When the id or the workspace has no saved session, the run stops before the
model is called:

```text
niki: no saved session "000000000000000018ebde128ab2bf5e-00" for /home/you/src/app; run without --resume to start one
```

## The `/threads` browser

`/threads` opens a filterable list of every saved session. A row shows the first
line of the title or of the first prompt, the stored message count and the
working directory the session was opened in. Choosing a row takes the same path
as `/resume <id>`.

```text
/threads
> deploy notes        12 messages   /home/you/src/app
  fix the parser      4 messages   /home/you/src/app
  (no prompt)         0 messages   /tmp/scratch
```

`/threads` refuses to open while a turn is running and says how to stop it. A
store that lists nothing prints `no sessions are saved yet`.

## `/resume`

Without an argument, `/resume` lists the newest 20 sessions and prints their
indices. With one, it accepts an index or the beginning of an id, rebuilds the
saved transcript into the view and makes that session the one the app writes
into. In this build the redrawn transcript is the view's only: the resumed
session is not replayed into the model's context.

```text
/resume
recent sessions; /resume <index> or /resume <id> opens one
1. 000000000000000018ebde128ab2bf5e-00 (12 messages); /home/you/src/app

/resume 1
resumed 000000000000000018ebde128ab2bf5e-00 (12 messages)
```

A target that matches nothing prints `there is no saved session 9; /resume
lists them` and leaves the current session alone. The [keyboard
reference](/keyboard) covers the `/threads` keys.

## `/rename`

`/rename <title>` names the current session. The title is trimmed before it is
stored. `/rename` without a title prints the current title, or `untitled`.

```text
/rename deploy notes
renamed to deploy notes

/rename
deploy notes
```

A headless run reaches the same command, and creates a session to name when the
workspace has none:

```bash
niki -n "/rename deploy notes"
```

## `/export`

`/export [path]` writes the session as Markdown and reports the path it wrote.
Without a path the file is `niki-<session-id>.md` in the workspace. A requested
path must resolve inside the workspace, and the parent directory must already
exist. An existing file is never replaced: the next export lands beside it as
`-2`, `-3`, and so on.

The document holds every prompt as `## You` and every answer as `## Niki`. Each
tool result is a fenced block labelled with the tool name, cut to 2 KiB and
marked `[truncated]` when it was longer; `tool_call` rows are not written. The
heading is the title, else the first prompt, else `Untitled session`.

```text
/export docs/run-notes.md
wrote /home/you/src/app/docs/run-notes.md
```

The headless form prints the path on stdout, so a script can capture it:

```bash
notes=$(niki -n "/export")
```

## `/clear`

`/clear` empties the view and starts a new session in the same workspace. The
old session is kept: it still appears in `/threads`, `/resume` and
`niki sessions list`, and it can be resumed by id. Approvals granted for the
previous session are dropped with it; the configured allow-list is untouched.

```text
/clear
started a fresh session
```

## `niki sessions list`

The command prints the saved sessions without opening the app. Rows come newest
stored write first, and the listing never writes to the database.

```bash
$ niki sessions list
000000000000000018ebde128ab2bf5e-00  1795773047  deploy notes  0 messages
```

A text row is the id, the stored update time in Unix seconds, the title (or
`(untitled)`) and the message count, separated by two spaces. An empty store
prints one line and exits 0:

```bash
$ niki sessions list
no saved sessions
```

`--json` prints one array instead, with these fields per session:

| Field | Type | Value |
| --- | --- | --- |
| `id` | string | The session id `--resume` takes. |
| `updated_at` | number | Unix seconds of the newest stored write. |
| `title` | string or null | The title `/rename` set, when there is one. |
| `messages` | number | Stored rows of every role. |
| `workspace` | string | The `cwd` the session was created in. |
| `archived` | boolean | Whether `niki sessions archive` hid the session. |

```bash
$ niki sessions list --json
[{"archived":false,"id":"000000000000000018ebde128ab2bf5e-00","messages":0,"title":"deploy notes","updated_at":1795773047,"workspace":"/tmp/tmp.CftaOuqUi2"}]
```

An empty store prints `[]`. `--root <PATH>` filters the list to sessions whose
stored `cwd` is exactly that path; the comparison is on the text that was
written, so pass the path the session recorded.

```bash
niki sessions list --root /home/you/src/app
niki sessions list --json --root /home/you/src/app
```

## `niki sessions fork` and `archive`

Two shell commands move a session's lifecycle without opening the app.
`fork` copies every stored message under a new id, so you can take one session
in a second direction without disturbing the first:

```bash
$ niki sessions list                 # find the id
000000000000000018ebde128ab2bf5e-00  1795773047  deploy notes  4 messages
$ niki sessions fork 000000000000000018ebde128ab2bf5e-00
000000000000000018ebde128ab2bf5e-01
```

`--json` prints one object instead, `{"from": "<old id>", "id": "<new id>"}`.
The copy keeps the title, the workspace and the messages; the two sessions then
evolve independently.

`archive` hides a session from every implicit picker — `--continue`, the
`/resume` list, the `/threads` browser and `niki sessions list` — while an
explicit id keeps working for `--resume <id>`, `fork` and a restore:

```bash
niki sessions archive 000000000000000018ebde128ab2bf5e-00
niki sessions archive --undo 000000000000000018ebde128ab2bf5e-00
niki sessions list --all             # archived rows end with "  [archived]"
```

`archive` prints nothing on success and exits 0; an unknown id prints
`niki: no saved session "<id>"; run niki sessions list` and exits 1.

## What `/compact` does

`/compact` shrinks the history the model is sent. It takes the turns before the
last six, sends them to the model with a fixed instruction to summarise them,
and replaces them with one `assistant` message that starts `Summary of the
earlier conversation:`. The last six turns stay verbatim, and a summary a
previous compaction left counts as a turn.

The app then folds earlier rows out of the transcript view and reports what
happened:

```text
/compact
compacted 18 messages into a 214 token summary; folded 9 earlier rows out of the view
```

When there is nothing older to fold, it prints `nothing to compact`. When the
summarising request fails, the history is left unchanged and the app prints
`compaction failed: <reason>`. `/context` shows the conversation's current size
and the session's token total.

`/compact` changes the request history, not the saved transcript: the rows in
`sessions.db` stay as they were, so an export or a later `/resume` still sees
the whole conversation. It does not touch the system prompt, the tool schemas or
the workspace files.

## Next

[Headless and scripted runs](/headless) covers `-n`, the output formats and
`niki doctor`, which are the other ways to reach a session without the app.
