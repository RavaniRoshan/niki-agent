---
name: commit
description: Write a commit for a change, with a Conventional Commits message that explains why the change exists.
---

# Writing a commit

Use this when the person asks you to commit, stage, or wrap up a change.

1. Read what changed: `git status`, `git diff`, and `git diff --staged`. Do not
   commit a file you did not touch, or one that belongs to another change in
   progress.
2. Stage the intended paths by name: `git add path/a path/b`. Never `git add -A`
   or `git commit -a`, which can sweep in build output or another session's work.
3. Write the subject as `type(scope): summary`, with a scope this repository
   allows. Text after the colon starts lowercase unless it begins with a proper
   noun or a code identifier; wrap identifiers in backticks.
4. Explain why in the body when the subject cannot: the failure that was seen,
   the behaviour that was wrong, or the trade-off that was chosen. Do not
   narrate the diff.
5. Run the repository's documented format, lint and test commands before
   committing. A failing check is a reason to fix the code, not to bypass the
   check.
6. Do not amend, rebase, or force-push a commit that has already been pushed.
   Leave a failing hook visible and fix its cause.
