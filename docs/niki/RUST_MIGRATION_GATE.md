# Rust Migration Gate

Python readiness must pass before Rust work begins. Gates:

- All P0 rows in CHECKLIST.md are WORKS with evidence.
- All package tests and network-free acceptance suite pass.
- Wheel retains upstream licenses/attribution.
- Matched Terminal-Bench 4.1/Harbor 0.25.0 protocol passes (Niki >= 55%, CI lower bound >= -10pp).
- Unified eval avg@3 no more than 5pp below baseline.
- Deterministic security/failure-injection gates pass.

Rust migration starts from Task 1 JSON event and port contracts. First phase keeps Python as temporary runtime behind a local protocol adapter.
