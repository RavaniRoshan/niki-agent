# Skills

A skill is a directory holding a `SKILL.md`: a YAML front-matter block with a
`name` and a `description`, then instructions for a recurring job. The model sees
the name, the description and, for a skill on disk, the path to read. It reads
the instructions only when the skill applies, so a long body costs nothing on a
turn that does not use it.

## Where skills live

Discovery takes the first match for a name, in this order:

| Precedence | Location | Origin |
| --- | --- | --- |
| 1 | `<workspace>/.niki/skills/<name>/SKILL.md` | project |
| 2 | `~/.niki/skills/<name>/SKILL.md` (or `$NIKI_HOME/skills`) | user |
| 3 | `commit`, `review` | built-in |

A project skill therefore hides a user skill of the same name, and both hide the
built-in. A skill directory is followed through a symlink, so a shared skills
directory can be linked into the profile rather than copied there.

A missing skills directory is normal and silent. A skill with no description, an
unterminated front matter, or a front-matter line that is not `key: value` is
skipped with a warning, and the rest still load. A headless run prints the
warning on stderr; the full-screen app adds one warning row to the transcript.

## Format

```markdown
---
name: release-notes
description: Write release notes from the commits since the last tag.
---

# Writing release notes

1. Read the commits since the last tag: `git log <tag>..HEAD`.
2. Group them by the reader's question, not by commit type.
3. Name what changed for the user and what did not change.
```

- The file must start with a `---` line and close the block with another.
- `name` is optional; the directory name is used when it is absent.
- `description` is required and non-empty. It is the one line the model sees
  before it decides whether to read the body.
- Only `name` and `description` are read from the front matter. Nested blocks
  and other keys are left alone, so a file written for a richer skill format
  still loads.
- Everything after the closing `---` is the instruction body.

## What the model sees

The skills section is appended to the system prompt on every request (and to
`--system` when you replace it):

```text
## Skills

Instruction packs for recurring jobs. Check this list before starting work;
when a skill applies, read its `SKILL.md` at the path shown and follow it. Do
not read a skill that does not apply.

- **release-notes**: Write release notes from the commits since the last tag.
  Read `/repo/.niki/skills/release-notes/SKILL.md` for the full instructions.
```

A description longer than 1024 characters is cut in the prompt; `/skills` shows
the full text. Built-in skills have no file to read, so their bodies are small
and are included in the section directly.

## `/skills`

`/skills` lists what discovery found, highest precedence first, with each
skill's origin. A broken skill is listed as a warning instead of taking the
session down. `/tools` lists the tool registry, not skills.

## Built-in skills

Two skills ship inside the binary:

| Name | Use it for |
| --- | --- |
| `commit` | Staging named paths and writing a Conventional Commits message that explains why the change exists. |
| `review` | Reviewing a branch, pull request or working tree for correctness, regressions and missing tests, with findings by severity. |

Both are lowest precedence: a project or user skill with the same name replaces
the built-in one.

## Trust

A skill is instruction text, not code. It cannot run anything by itself. It can,
however, direct the model to call a tool, and those calls meet the same policy
gate as any other turn — so a skill that asks for a shell command still needs the
command allow-listed, approved, or run in YOLO. Review a project's
`.niki/skills/` before you run Niki there.

There is no plugin loader and no extension API beyond skills, hooks and MCP in
this build; a skill and a `hooks.json` are the surfaces a repository can carry.
