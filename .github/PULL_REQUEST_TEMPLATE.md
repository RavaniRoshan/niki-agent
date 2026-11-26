<!-- Optional: replace this comment with `Fixes #`, `Closes #`, or `Resolves #` and the issue number to auto-close the issue when this PR is merged. If applicable, add separate `Depends on #` or `Related: #` lines. -->

<!-- For a net-new feature or behavior-changing bugfix: replace this comment with one plain-English sentence on the user-visible change, keeping the `---` divider below it. For chores, refactors, and test-only PRs, no summary is needed — delete the `---` divider along with this comment so the body starts with the rationale below.

With a summary, the body should look like:

    One plain-English sentence on the user-visible change.

    ---

    Why this change, and why this approach.
-->

---

<!-- Why this change, and why this approach. No `# Summary` or `## Release note` headings, please. -->

Thanks for contributing. A few essentials, then remove this section before
submitting.

1. **Title:** `type(scope): description` — e.g. `fix(sdk): ...`, `feat(code): ...`.
   Allowed types and scopes are defined in
   [`.github/workflows/pr_lint.yml`](workflows/pr_lint.yml).
2. **One package per PR** unless a maintainer says otherwise.
3. **External contributors:** do not change `uv.lock` or add dependencies
   without maintainer approval.
4. **Large pasted AI descriptions** may be ignored or closed. A short rationale
   written by you is more useful than a generated wall of text.
5. **English only.**

Run the local checks before pushing:

```bash
cd libs/code
make check
```

The full contributor guide is [`AGENTS.md`](../AGENTS.md).
