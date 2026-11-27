# Engineering rules

Behaviour the maintainers have settled on, one sentence each. Every crate keeps
every rule here. Add one when a correction is made, so the next change cannot
repeat the mistake.

## Product

1. The public surface never names another agent product or its runtime; attribution lives only in `NOTICE`, `LICENSE`, and vendored provenance files.
2. The shipped binary is `niki`; there are no aliases for other names.
3. A user-visible string is written for the person running the command, not for the person who wrote the code.

## Behavior

4. A recoverable failure returns `Result`; a panic is a defect unless the state is genuinely impossible.
5. A provider failure is reported with the provider name and the HTTP status, never as a generic "request failed".
6. Cancelling a turn stops new work immediately and keeps the partial transcript.
7. The workspace root is a hard boundary: a tool path that resolves outside it is refused, not clamped.

## Tests

8. No test calls a live model provider; fixtures live beside the crate that uses them.
9. A fixture is reviewed like code: a provider change needs a new fixture, and editing a fixture is never a way to make a test pass.
10. Every normalization used by replay is documented where it is applied, with its reason.

## Style

11. No `unsafe` outside vendored code; when unavoidable, one `// SAFETY:` comment stating the invariant.
12. Comments explain why, never what; a comment that restates the code is deleted.
13. `#[expect(lint, reason = "...")]`, never `#[allow]`, so a stale suppression is flagged.
