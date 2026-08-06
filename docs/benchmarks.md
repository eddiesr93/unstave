# Performance benchmark

Measured 2026-08-06 on an Apple M4 Pro MacBook Pro (14 cores, 24 GB RAM),
macOS 26.5.2, using Rust 1.97.1 in the release/bench profile.

## 6000-file acceptance benchmark

`cargo bench -p unstave-core --bench pipeline` generates a fresh workspace with
6000 TypeScript modules, including 480 definitions behind a barrel and 5518
consumers. Generation time is excluded from the analysis measurement.

| Run | Actual | Budget | Result |
|---|---:|---:|---|
| Cold, cache miss | 234.0 ms | < 1500 ms | pass |
| Warm, content-hash cache hit | 117.4 ms | < 200 ms | pass |

Median of three consecutive runs. The benchmark executable exits non-zero when
either budget is exceeded. It also asserts the exact module count and verifies
that the warm run was a real cache hit.

The generated tree is deliberately wide and shallow — 6000 files across about 40
directories — so it exercises parsing far more than directory traversal. Real
workspaces are deeper, which is where the parallel walk matters; see below.

## Real workspace

A private 5129-module Vite/React application, analyzed with the release CLI.
Phase timings come from the report's own `timings` block, median of three runs.

| Phase | Before parallel discovery | After |
|---|---:|---:|
| Discovery | 170 ms | 45 ms |
| Warm total | 327 ms | 129 ms |

The warm path re-walks the workspace and re-hashes file contents to decide
whether the cache is still valid, so discovery is paid on every run, hit or
miss. Making the walk parallel is what brought the warm total under the 200 ms
budget on a real tree. Cache load itself is 9 ms for an 11 MB archive; the
fingerprint pass over 11.8 MB of sources is about 75 ms.

These are observed measurements, not estimates.
