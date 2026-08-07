# What barrel files actually cost, measured on Vite, TanStack Query, and Astro

Every TypeScript codebase of a certain age has them. A directory grows past a
handful of files, someone adds an `index.ts` that re-exports everything in it,
and from then on the rest of the codebase imports from the directory instead of
from the file:

```ts
import { Client0 } from '@/clients';
```

The import looks like it costs one module. It rarely does.

A barrel is a single graph node that depends on everything behind it. Ask it for
one symbol and you get the whole directory — and, if the modules behind it import
from other barrels, whatever those pull in too. Your production bundler
tree-shakes this away, so the shipped output is fine. That is exactly why the
problem survives for years: it is invisible in the metrics teams actually watch.

Where it is not invisible is in the tools that do not tree-shake. A dev server
resolves and transforms the graph eagerly. A type checker walks all of it. Both
of them pay for every module the barrel dragged in, on every cold start.

The interesting question is not whether this is true in principle. It is: how
much does it cost in real, well-maintained repositories, and can a tool remove
that cost without breaking the code?

## The instrument

[unstave](https://github.com/eddiesr93/unstave) is a Rust CLI built on
[oxc](https://oxc.rs). It builds the module graph of a TypeScript workspace, then
answers a specific question for each import site: how many modules does this
import actually reach, and how many would it reach if it pointed directly at the
module that declares the symbol?

The ratio between those two numbers is what unstave calls amplification. The
difference is excess — modules pulled in for no reason.

```console
$ unstave barrels

Barrel amplification
┌──────────────────────┬───────┬──────┬─────────┬───────┬────────┬────────────┐
│ barrel               ┆ sites ┆ cost ┆ excess  ┆ worst ┆ amp    ┆ rewritable │
╞══════════════════════╪═══════╪══════╪═════════╪═══════╪════════╪════════════╡
│ src/clients/index.ts ┆ 4920  ┆ 1047 ┆ 5136480 ┆ 1044  ┆ 349.0× ┆ 4920/4920  │
└──────────────────────┴───────┴──────┴─────────┴───────┴────────┴────────────┘
```

It is not a bundler and not a linter. It runs no `tsc`, does no type inference,
and says nothing about bundle size. It measures one thing and rewrites one thing.

The rewriting part is where a tool like this earns or loses trust, so it is
deliberately timid. `unstave fix` dry-runs by default and prints a unified diff.
It only touches the bytes inside the import statements it rewrites — no
reformatting, no reordering, no reflowing the rest of the file. Imports it cannot
resolve unambiguously, namespace imports, cyclic or external re-exports, and
barrels with observed side effects are left byte-identical and reported with a
reason.

## Three real repositories

Synthetic benchmarks prove a tool is fast. They do not prove it is right about
code other people wrote. So the v0.2.1 validation ran against pinned commits of
three public TypeScript projects, chosen because they are large, actively
maintained, and not written to make this tool look good.

| Project | Modules | Cache miss | Cache hit | Peak RSS | Strongest targeted barrel |
|---|---:|---:|---:|---:|---|
| [Vite](https://github.com/vitejs/vite) | 1,546 | 68 ms | 38 ms | 20.9 MiB | `module-runner/index.ts`: 5x peak, 18 excess |
| [TanStack Query](https://github.com/TanStack/query) | 1,081 | 41 ms | 26 ms | 21.0 MiB | `svelte-query/src/index.ts`: 19x peak, 189 excess |
| [Astro](https://github.com/withastro/astro) | 2,858 | 154 ms | 93 ms | 44.7 MiB | `app/entrypoints/index.ts`: 143x peak, 574 excess |

Measured 2026-08-07 on an Apple M4 Pro (14 cores, 24 GB RAM), macOS 26.5.2.
Timings are median internal `totalMs` across three consecutive pairs. A cache
miss means the on-disk cache was deleted first; it does not mean the OS page
cache was flushed.

Two things stand out.

The first is that whole-workspace analysis of a 2,858-module monorepo takes 93 ms
warm and under 45 MiB of memory. That is fast enough to run on every save rather
than in a nightly job, which is the difference between a metric people act on and
one they read once.

The second is the shape of the amplification. Astro's worst site reaches **143
modules for one import**. Not a pathological fixture — the app entrypoints barrel
in a real framework, doing exactly what barrels are supposed to do.

## What the rewrites did

Finding a number is easy. The load-bearing claim is that the rewrite is safe, and
that has to be checked against each project's own toolchain, not against
unstave's assertion that it went fine.

**Vite.** One import of `ESModulesEvaluator` reached 20 modules where the direct
definition path needs four: 5x amplification, 16 excess at that site. The
targeted plan rewrote five imports across four files. After a frozen,
package-filtered install, Vite's own package build and typecheck completed, and
the directly affected server source map suite passed all 10 tests. Re-analysis
dropped the barrel out of the ranking and did not increase unresolved specifiers.

**TanStack Query.** `packages/svelte-query/src/index.ts` reached 19 modules at
its worst site and accumulated 189 excess across all its sites. The plan rewrote
14 imports in 14 test files and skipped four symbols it could not prove — the
skips matter as much as the rewrites. Afterwards the two remaining sites sat at
1x amplification and zero excess. `svelte-check` reported zero errors; the
package suite passed 168 tests across 23 files including Vitest type checking;
ESLint reported zero errors on every rewritten file.

This one also needed a flag: `--condition @tanstack/custom-condition`. Monorepos
frequently point their `exports` maps at build output, so a source-graph tool
that does not know the project's source condition will fail to resolve
cross-package imports and understate every number behind them. If unstave reports
a suspicious pile of unresolved specifiers in your workspace, that is almost
always the cause.

**Astro.** The 143x barrel. Five imports rewritten across five source files, the
filtered dependency build completed, and the 84 tests closest to the rewritten
modules — manifest, routing, app — passed. A wider unit run passed 3,271 of
3,276; three failures and one cancellation needed fixtures outside the filtered
install or live font downloads, and were unrelated to the rewrites. Those are
disclosed in the validation document rather than quietly dropped from the
summary.

## What this does not prove

Being precise about the boundary is the whole point of publishing numbers.

**These are source-graph savings, not build-time savings.** Removing 574 excess
modules from a barrel's import sites shrinks the graph a dev server and type
checker have to traverse. It does not mean your build got proportionally faster,
and nothing here measured application startup or production build duration. Any
claim of the second kind would need a different experiment.

**The clones were filtered.** Dependencies came from each pinned lockfile with a
package filter for the affected workspace, rather than building every package in
very large monorepos. That is also why unresolved-specifier and dead-export
totals are excluded from the comparison: a filtered install cannot make every
fixture and workspace package available.

**Your numbers depend entirely on how your barrels are shaped.** The synthetic
6000-module workspace in the repo shows a 349x barrel because it was built to.
Vite showed 5x. Same tool, same analysis, two orders of magnitude apart. There is
no useful average here — there is only what your own graph looks like.

Every write test ran inside a disposable clone. `git diff --check` passed, each
repository's own formatter accepted every rewritten file, and re-analysis showed
no increase in unresolved specifiers.

## The bug the real repositories found

Running against real code found something the test suite did not. Astro and
TanStack Query use NodeNext-style imports with explicit runtime extensions:

```ts
import { thing } from './thing.js';
```

The codemod was rewriting the specifier to the source file it resolved to, which
produced imports that typechecked but would not run. It also normalized
semicolons, which meant a semicolon-free codebase got a diff its formatter
immediately disagreed with — small, but exactly the kind of noise that makes a
team stop trusting a codemod.

Commit [`0128133`](https://github.com/eddiesr93/unstave/commit/0128133) added a
regression fixture and now preserves explicit `.js`, `.mjs`, and `.cjs`
specifiers along with the source file's semicolon convention. That fix is the
most valuable thing to come out of this validation, and it is a good argument for
running any codemod against foreign code before believing it.

## Reproducing this

Nothing above is a claim you have to take on trust. The full methodology, pinned
commit hashes, boundaries, and before/after results are in
[`docs/real-world-validation.md`](https://github.com/eddiesr93/unstave/blob/main/docs/real-world-validation.md).

```sh
git clone https://github.com/eddiesr93/unstave.git
cd unstave
git checkout 0128133
cargo build --release -p unstave-cli

git clone https://github.com/vitejs/vite.git /tmp/unstave-vite
git -C /tmp/unstave-vite checkout 57fea001d154e7dd8d5d74d3082731f1dcfd31be

./target/release/unstave cache clear --root /tmp/unstave-vite
./target/release/unstave analyze --root /tmp/unstave-vite --format json --out /tmp/unstave-vite-report
./target/release/unstave fix --root /tmp/unstave-vite \
  --barrel packages/vite/src/module-runner/index.ts --dry-run
```

## Trying it on your own workspace

```bash
cargo install unstave-cli
```

```bash
brew tap eddiesr93/unstave https://github.com/eddiesr93/unstave
brew install unstave
```

Then, from your workspace root:

```bash
unstave barrels
```

That is the whole first step. It writes nothing and tells you whether you have a
problem worth acting on. If you do, `unstave analyze --format html` produces a
single portable report with no CDN or network dependency — there is a
[live sample](https://eddiesr93.github.io/unstave/sample-report.html) — and
`unstave fix` will show you the diff before it touches anything.

There is also a Vite plugin that runs the analysis asynchronously, so startup and
HMR never wait on it, and serves the live report at `/__unstave` in development:

```bash
npm install --save-dev @unstave/vite-plugin
```

unstave is MIT-licensed and early: v0.2.1, with interfaces that may still change
before 1.0. If you run it and find something real — or something wrong — the
[Show and tell](https://github.com/eddiesr93/unstave/discussions/categories/show-and-tell)
and [Q&A](https://github.com/eddiesr93/unstave/discussions/categories/q-a)
discussion categories are the right place for it. Module count, top barrel,
amplification, and what you changed is the most useful shape for a report.

[Documentation](https://eddiesr93.github.io/unstave/docs/) ·
[Repository](https://github.com/eddiesr93/unstave) ·
[crates.io](https://crates.io/crates/unstave-cli) ·
[npm](https://www.npmjs.com/package/@unstave/node)
