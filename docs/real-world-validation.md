# Real-world validation

Measured 2026-08-07 on an Apple M4 Pro MacBook Pro (14 cores, 24 GB RAM),
macOS 26.5.2, using the release build from commit `0128133`.

The goal of this validation is narrower than a bundler benchmark: prove that
unstave can discover, rank, and safely rewrite barrel imports in active public
TypeScript repositories. The numbers below describe unstave's graph model. They
do not claim an equivalent reduction in application startup or build time.

## Results

| Project | Pinned commit | Modules | Cache miss | Cache hit | Peak RSS | Strongest targeted barrel | Targeted rewrite |
|---|---|---:|---:|---:|---:|---|---|
| [Vite](https://github.com/vitejs/vite) | [`57fea00`](https://github.com/vitejs/vite/commit/57fea001d154e7dd8d5d74d3082731f1dcfd31be) | 1,546 | 68 ms | 38 ms | 20.9 MiB | `module-runner/index.ts`: 5x peak, 18 total excess | 4 files, 5 imports |
| [TanStack Query](https://github.com/TanStack/query) | [`46d7f02`](https://github.com/TanStack/query/commit/46d7f02f1c7b9fcd3255082cc7103e8bfa3dab76) | 1,081 | 41 ms | 26 ms | 21.0 MiB | `svelte-query/src/index.ts`: 19x peak, 189 total excess | 14 files, 14 imports |
| [Astro](https://github.com/withastro/astro) | [`fba468c`](https://github.com/withastro/astro/commit/fba468c228d8661d2383a80c74206075201a187b) | 2,858 | 154 ms | 93 ms | 44.7 MiB | `app/entrypoints/index.ts`: 143x peak, 574 total excess | 5 files, 5 imports |

Cache miss and cache hit are the median internal `totalMs` values from three
consecutive pairs. A cache miss means `.unstave/cache-v1.rkyv` was removed before
the run; it does not mean the operating-system page cache was flushed. Peak RSS
is one separate cache-hit run measured with macOS `/usr/bin/time -l`.

## What unstave found

### Vite

One import of `ESModulesEvaluator` from `packages/vite/src/module-runner/index.ts`
reached 20 modules instead of the four needed by the direct definition path: 5x
amplification and 16 excess modules at that site. A targeted codemod plan rewrote
five imports across four files. Re-analysis removed that barrel from the imported
barrel ranking and did not increase unresolved specifiers.

### TanStack Query

`packages/svelte-query/src/index.ts` reached 19 modules at its most amplified
site and accumulated 189 excess modules across its import sites. The targeted
plan rewrote 14 safe imports in 14 test files and skipped four symbols it could
not prove. After applying the plan, the two remaining sites had 1x amplification
and zero excess. The command used the repository's source export condition:
`--condition @tanstack/custom-condition`.

### Astro

`packages/astro/src/core/app/entrypoints/index.ts` reached 143 modules at its
worst site and accumulated 574 excess modules. The targeted plan rewrote five
imports across five source files. Re-analysis removed that barrel from the
imported barrel ranking and did not increase unresolved specifiers.

## Reproduction

Build the exact unstave revision and clone each project at the pinned commit:

```sh
git clone https://github.com/eddiesr93/unstave.git
cd unstave
git checkout 0128133
cargo build --release -p unstave-cli

git clone https://github.com/vitejs/vite.git /tmp/unstave-vite
git -C /tmp/unstave-vite checkout 57fea001d154e7dd8d5d74d3082731f1dcfd31be

./target/release/unstave cache clear --root /tmp/unstave-vite
./target/release/unstave analyze \
  --root /tmp/unstave-vite \
  --format json \
  --out /tmp/unstave-vite-report
./target/release/unstave fix \
  --root /tmp/unstave-vite \
  --barrel packages/vite/src/module-runner/index.ts \
  --dry-run
```

Repeat `analyze` without clearing the cache for a cache-hit measurement. For
TanStack Query, add `--condition @tanstack/custom-condition` to both `analyze`
and `fix`.

## Validation boundaries

- The repositories were shallow clones with no dependency installation. This
  keeps the test fast and isolates source-graph analysis, but bare third-party
  imports remain unresolved.
- Because dependencies were absent, unresolved-specifier and dead-export totals
  are deliberately excluded from the comparison. The barrel findings above use
  internal source paths that resolved in both the original and rewritten trees.
- Parse failures were limited to intentional syntax-error fixtures already
  present in the upstream repositories.
- Every write test ran only inside a disposable clone. `git diff --check` passed,
  and re-analysis showed no increase in unresolved specifiers.
- The live-project run exposed a NodeNext style gap in the codemod. Commit
  [`0128133`](https://github.com/eddiesr93/unstave/commit/0128133) added a
  regression fixture and now preserves explicit `.js`, `.mjs`, and `.cjs`
  specifiers plus the source file's semicolon convention.
