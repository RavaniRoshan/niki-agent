# First post-Task-11 baseline

The planned pre-Task-1 baseline was missed and cannot be reconstructed. This document is reserved for the first post-Task-11 matched Terminal-Bench capture; live runs are NOT RUN in this implementation pass.

## Current Python completion baseline — NOT RUN

Checked on 2026-11-24 before any new product behavior change:
`ANTHROPIC_API_KEY`, `LANGSMITH_API_KEY`, and the other provider keys are unset;
no Docker runtime is available and the earlier Podman check failed on a read-only
runtime path; the branch is local with no pushed commit for a workflow dispatch.

The unified `autonomous`/`dcode` `lite` regression eval therefore has **no
measured avg@3**. The `unified avg@3 no more than 5pp below baseline` gate is
blocked and stays blocked; no score is estimated or substituted. Re-run the gate
from Task 3 Step 2 only with credentials, a working sandbox runtime, and a
published branch, recording the exact evaluated SHA.
