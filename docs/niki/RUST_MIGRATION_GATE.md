# Rust Migration Gate

Python readiness must pass before Rust work begins. Gates:

- All P0 rows in CHECKLIST.md are WORKS with evidence.
- All package tests and network-free acceptance suite pass.
- Wheel retains upstream licenses/attribution.
- Deterministic security/failure-injection gates pass.

That is the whole gate. A matched comparison against Claude Code is **not** a
precondition, by owner decision on 2026-11-26: the Harbor/Terminal-Bench rig
cannot run in this environment (no container runtime, no hosted credentials),
and tying the port to it stalled the product instead of protecting it. The
comparison is deferred to a later session, where it can be run in an
environment that has a working sandbox runtime and keys; `EVALUATION.md` keeps
the protocol so that session starts from a written plan rather than memory.

Rust migration starts from the JSON event and port contracts, not from the
Python source. The first phase keeps Python as the temporary agent runtime
behind a local protocol adapter.
