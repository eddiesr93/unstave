# Performance benchmark

Measured 2026-08-05 on an Apple M4 Pro MacBook Pro (14 cores, 24 GB RAM),
macOS 26.5.2, using Rust 1.95.0 in the release/bench profile.

## 6000-file acceptance benchmark

`cargo bench -p unstave-core --bench pipeline` generates a fresh workspace with
6000 TypeScript modules, including 480 definitions behind a barrel and 5518
consumers. Generation time is excluded from the analysis measurement.

| Run | Actual | Budget | Result |
|---|---:|---:|---|
| Cold, cache miss | 182.2 ms | < 1500 ms | pass |
| Warm, content-hash cache hit | 90.7 ms | < 200 ms | pass |

The benchmark executable exits non-zero when either budget is exceeded. It also
asserts the exact module count and verifies that the warm run was a real cache
hit.

## End-to-end CLI check

The larger pre-existing synthetic generator produces 6002 modules and a 5.7 MB
JSON report. Running the release CLI, including graph analyses, JSON rendering,
and writing that report, measured:

| Run | Wall time | Cache state |
|---|---:|---|
| Cold | 0.51 s | miss |
| Warm | 0.11 s | hit |

These are observed single-run wall-clock measurements, not estimates. They are
kept separate from the guarded core benchmark because output format and disk
write size are caller-controlled costs.
