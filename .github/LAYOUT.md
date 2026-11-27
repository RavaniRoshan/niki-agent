# `.github` layout

One product, one pipeline. Every workflow below is live.

| Workflow | What it does |
| --- | --- |
| `ci.yml` | Runs the Rust workspace gate; publishes the aggregate **✅ CI Success** check. |
| `niki-release.yml` | On a `v*` tag or manual dispatch, builds `niki` for Linux, macOS (arm64 and x86_64) and Windows, runs the installer and a smoke turn, and publishes a GitHub Release. |
| `pr_lint.yml` | Validates Conventional Commits titles and the allowed scope list. |
| `pr_lint_trailer.yml` | Checks the required commit trailer. |
| `branch_name_check.yml` | Enforces `<user>/<scope>/<description>` branch names. |
| `block_fork_main_prs.yml` | Blocks PRs whose head ref is a fork's `main`/`master`. |
| `markdown_file_check.yml` | Validates Markdown files; a non-docs PR adding Markdown needs the `ci:ack-markdown` label. |
| `pr_labeler.yml` | Applies labels from changed paths. |
| `pr_scope_file_check.yml` | Keeps a PR's changed paths inside its declared scope. |
| `require_issue_link.yml` | Requires an issue link on external PRs. |
| `close_old_prs.yml`, `close_unchecked_issues.yml`, `keep_open_on_comment.yml`, `waiting_on_author*.yml`, `reopen_on_assignment.yml`, `clear_pending_deletion.yml`, `sync_priority_labels.yml`, `sync_topic_labels.yml`, `ripgrep_timeout_comment.yml`, `tag-external-issues.yml` | Housekeeping automation. |

## Scripts

- `.github/scripts/checks/` — repo gates: the branch-name/scope sync check and
  the Markdown gate.
- `.github/scripts/release/` — release-note and freshness helpers. The
  release-please fan-out scripts are retained for history but are no longer
  wired to a workflow.
- `.github/scripts/labeling/` — PR label and scope helpers.

## Branching

The default branch is `niki-agent`. There is no `main`.
