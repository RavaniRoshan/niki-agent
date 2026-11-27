---
name: review
description: Review a change for correctness, regressions and missing tests before it merges.
---

# Reviewing a change

Use this when the person asks for a review of a branch, a pull request, or the
working tree.

1. Read the change against its stated intent: the issue text, the commit
   messages, and the diff. Read the file around a hunk before judging it; a line
   without its context is not a finding.
2. Check the behaviour in the form the person will use it: run the repository's
   build and test commands, not only the tests the change adds.
3. Look for what a green run does not show: an error path that panics, a
   user-visible string that names another product instead of Niki Agent, a
   boundary the code can escape, or a test that passes because it asserts
   nothing.
4. Report each finding with `path/to/file:42`, the failure that follows from it,
   and the smallest change that fixes it. Rank by severity; do not bury a
   blocker under style notes.
5. Say what you could not verify. "Not run" is a finding; "looks fine" about a
   file you did not read is not.
