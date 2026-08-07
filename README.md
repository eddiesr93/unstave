# unstave

[![crates.io](https://img.shields.io/crates/v/unstave-cli.svg)](https://crates.io/crates/unstave-cli)
[![npm](https://img.shields.io/npm/v/@unstave/node.svg?color=cb3837&logo=npm)](https://www.npmjs.com/package/@unstave/node)
[![CI](https://img.shields.io/github/actions/workflow/status/eddiesr93/unstave/ci.yml?branch=main&label=CI)](https://github.com/eddiesr93/unstave/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/github/license/eddiesr93/unstave.svg)](https://github.com/eddiesr93/unstave/blob/main/LICENSE)
[![GitHub stars](https://img.shields.io/github/stars/eddiesr93/unstave.svg?style=social)](https://github.com/eddiesr93/unstave)

**Your `index.ts` barrel files are why your dev server takes 20 seconds to start.
`unstave` proves it, then fixes it.**

[Product site](https://eddiesr93.github.io/unstave/) · [Documentation](https://eddiesr93.github.io/unstave/docs/) · [Installation](#install) · [Usage](#usage) · [Changelog](CHANGELOG.md) · [crates.io](https://crates.io/crates/unstave-cli) · [npm](https://www.npmjs.com/package/@unstave/node)

One innocent-looking import:

```ts
import { Client0 } from '@/clients';
```

pulls **1047 modules** into your dev server's graph. It needs **3**.

```console
$ unstave barrels

Barrel amplification
┌──────────────────────┬───────┬──────┬─────────┬───────┬────────┬────────────┐
│ barrel               ┆ sites ┆ cost ┆ excess  ┆ worst ┆ amp    ┆ rewritable │
╞══════════════════════╪═══════╪══════╪═════════╪═══════╪════════╪════════════╡
│ src/clients/index.ts ┆ 4920  ┆ 1047 ┆ 5136480 ┆ 1044  ┆ 349.0× ┆ 4920/4920  │
└──────────────────────┴───────┴──────┴─────────┴───────┴────────┴────────────┘

Projected per-entrypoint module count after a full codemod
┌──────────────┬────────┬───────┬───────────┐
│ entrypoint   ┆ before ┆ after ┆ removed   │
╞══════════════╪════════╪═══════╪═══════════╡
│ src/main.tsx ┆ 1128   ┆ 195   ┆ 933 (83%) │
└──────────────┴────────┴───────┴───────────┘
```

Then `unstave fix --write` rewrites those imports to point at the modules that
actually declare the symbols — **without reformatting a single byte** outside the
import statements it touches.

Analysing 6000 files takes **117.4 ms** warm and **234.0 ms** on a cold page cache
(median of three runs on an Apple M4 Pro, 14 cores).

<sub>Numbers above are from the reproducible benchmark workspace included in this
repo, not from a customer codebase. Generate it yourself and check the arithmetic:
`cargo run --release -p unstave-core --example gen_synthetic -- /tmp/demo 6000`.
Your own numbers depend entirely on how your barrels are shaped — run
`unstave barrels` and find out.</sub>

> **Seen in the wild:** during development, unstave was run against the real
> [`withastro/astro`](https://github.com/withastro/astro) repository. The v0.2.0
> fix for bare barrel imports cut `src/core/config/index.ts` from **167 reported
> excess to 65**, and `dev-toolbar/ui-library/index.ts` from **13 to 0**
> ([changelog](CHANGELOG.md)).

Fresh validation against pinned public revisions of Vite, TanStack Query, and
Astro analyzed **1,081–2,858 modules in 26–93 ms warm**. Targeted dry runs found
safe rewrites across all three repositories, including a **143x** Astro import
site. The rewritten code then passed Vite's build and typecheck, 168 TanStack
Query tests, and 84 targeted Astro tests. The complete methodology, commit
hashes, limitations, and before/after results are in the
[real-world validation](docs/real-world-validation.md).

If unstave surfaces a real bottleneck in your project,
[star the repository](https://github.com/eddiesr93/unstave) so other TypeScript
teams can find it.

## Why this happens

A barrel (`index.ts` that re-exports a directory) is a single node that depends on
everything behind it. Your bundler tree-shakes it for production, so the shipped
output is fine — but your **dev server** resolves and transforms the whole graph
eagerly, and your **type checker** walks all of it. That cost is invisible in
production metrics, which is why it survives for years.

`unstave` is the measuring instrument for that cost, and the codemod that removes it.

> **Status: early.** The initial implementation is complete and release automation
> is in place. Interfaces may still change before 1.0.

## What it is not

Not a bundler, not a linter. It does no TypeScript type checking — no `tsc`, no type
inference — and no bundle-size analysis.

## Install

After publishing a version tag, install the CLI from crates.io:

```bash
cargo install unstave-cli
```

Or install it with Homebrew:

```bash
brew tap eddiesr93/unstave https://github.com/eddiesr93/unstave
brew install unstave
```

The native Node API and Vite integration are published separately. The Vite plugin
depends on the native API, so installing it pulls `@unstave/node` in as well:

```bash
npm install --save-dev @unstave/vite-plugin
pnpm add -D @unstave/vite-plugin
yarn add -D @unstave/vite-plugin
bun add -d @unstave/vite-plugin
```

Install `@unstave/node` on its own when you call `analyze` directly:

```bash
npm install @unstave/node
pnpm add @unstave/node
yarn add @unstave/node
bun add @unstave/node
```

## Usage

```bash
unstave analyze --format terminal
unstave analyze --format json --format html --out .unstave
```

```
unstave analyze [--format terminal|json|dot|mermaid|html]... [--out <dir>] [--max-nodes <n>]
unstave barrels [--min-amplification <f>]
unstave cycles
unstave dead-exports
unstave fix [--barrel <path>] [--only <glob>] [--import-style alias|relative|preserve]
            [--dry-run|--write|--check]
```

Global flags: `--config <path>`, `--root <path>`, `--no-cache`, `-v/-vv`.

Analysis uses a checked, content-addressed cache at
`<root>/.unstave/cache-v1.rkyv`. Source contents, configuration, manifests,
tsconfig files, and lockfiles participate in the key. Use `--no-cache` for a
forced cold run and `unstave cache clear` to remove the exact cache file.

Non-terminal formats are written as `unstave-report.json`, `.dot`, `.mmd`, and
`.html`. The default directory is `<root>/.unstave`; `--out` overrides it. JSON is
complete and versioned with `schemaVersion: 2`. The HTML report is a single portable
file with no CDN or runtime network dependency. All three graph formats — HTML, DOT,
and Mermaid — group modules by directory and collapse large graphs to at most
`--max-nodes` directory nodes (default: 150). Click a collapsed node in the HTML
report to list the modules behind it.

#### HTML report preview

![Interactive HTML report generated by `unstave analyze --format html`](./assets/report-preview.png)

Real output, produced by running the CLI against this repo's `nested-barrels`
test fixture — a barrel that pulls in six modules for one imported symbol. Open
it live: [**sample-report.html**](https://eddiesr93.github.io/unstave/sample-report.html).

`unstave fix` is conservative and dry-runs by default: it prints a unified diff and
does not touch source files. Use `--write` to apply the same plan, or `--check` in CI
to exit with status 1 when a rewrite is needed. `--only` limits importers by a
workspace-relative glob, while `--barrel` limits rewrites to one barrel. Imports
that cannot be resolved unambiguously, namespace imports, cyclic/external
re-exports, and barrels with observed side effects are left byte-identical and
reported by reason.

```bash
# Preview every safe rewrite.
unstave fix --root .

# Rewrite one barrel in application sources, keeping the predominant path style.
unstave fix --root . --barrel src/clients/index.ts --only 'src/app/**' --write

# Fail CI if source files are not yet direct-imported.
unstave fix --root . --check
```

## Configuration

Optional `unstave.toml` at the workspace root. Every field has a default, and CLI
flags override the file.

```toml
entrypoints = ["src/main.tsx"]
include = ["**/*.{ts,tsx}"]
exclude = ["**/*.test.ts", "**/*.stories.tsx"]

[barrel]
reexport_ratio = 0.8   # re-exports must be this fraction of all exports
max_own_decls = 2      # ...and the module may declare at most this many things

[codemod]
import_style = "preserve"

[resolve]
# Extra export conditions, tried before the defaults.
conditions = ["@tanstack/custom-condition"]
```

### Monorepos: export conditions

If `unstave analyze` reports a large number of unresolved specifiers in a
workspace, the usual cause is that your packages' `exports` maps point at build
output that has not been built yet:

```json
"exports": { ".": { "import": "./dist/index.js" } }
```

Many monorepos add a custom condition pointing at TypeScript source, so that
development resolves to `src/` while consumers get `dist/`. Pass it with
`--condition` (repeatable) or set `[resolve] conditions` in `unstave.toml`:

```bash
unstave barrels --condition @tanstack/custom-condition
```

Common values are `development`, `source`, and project-specific names. Conditions
are tried **before** the defaults, so a source-pointing condition wins over the
`import`/`default` entry.

If your packages expose no source condition at all, build the workspace first —
otherwise cross-package imports cannot resolve and every count behind them will be
understated.

## Vite plugin

`@unstave/vite-plugin` runs the native analysis asynchronously, so Vite startup and
HMR do not wait for the module graph. In development it serves the live report at
`/__unstave`; production analysis is opt-in and writes JSON plus HTML at `buildEnd`.

```ts
import { defineConfig } from 'vite'
import unstave from '@unstave/vite-plugin'

export default defineConfig({
  plugins: [
    unstave({
      warnAmplification: 5,
      serveReport: true,
      outDir: '.unstave',
    }),
  ],
})
```

Set `enabled: true` to generate reports during production builds, or
`enabled: false` to disable the plugin explicitly.

## Crates

| Crate | Purpose |
|---|---|
| `unstave-core` | Discovery, parsing, graph construction, analyses. A library — no I/O policy, no printing. |
| `unstave-report` | Terminal, JSON, DOT, Mermaid and HTML renderers. |
| `unstave-codemod` | Span-based, byte-preserving plans for safe barrel-import rewrites. |
| `unstave-napi` | Two-function asynchronous Node-API boundary for reports and HTML. |
| `unstave-cli` | The `unstave` binary. |

## Development

```bash
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all
```

The release benchmark generates 6000 TypeScript modules and enforces the
benchmark budgets — cold cache miss under 1500 ms, warm content-hash cache hit
under 200 ms:

```bash
cargo bench -p unstave-core --bench pipeline
```

See [docs/benchmarks.md](docs/benchmarks.md) for the synthetic performance
benchmark and [docs/real-world-validation.md](docs/real-world-validation.md) for
pinned Vite, TanStack Query, and Astro results.

Built on [oxc](https://oxc.rs) for parsing and resolution. The `oxc_*` crates release
in lockstep, so they are pinned to a single exact version across the workspace —
mixing versions breaks the AST.

## Community

- [Share a result](https://github.com/eddiesr93/unstave/discussions/categories/show-and-tell)
  from a real workspace: module count, top barrel, amplification, and the change
  you made. Remove proprietary paths and source code first.
- [Ask a question](https://github.com/eddiesr93/unstave/discussions/categories/q-a)
  when you need help interpreting a report or configuring an integration.
- [Propose an idea](https://github.com/eddiesr93/unstave/discussions/categories/ideas)
  before turning a broad workflow improvement into an implementation issue.
- Use a [bug report](https://github.com/eddiesr93/unstave/issues/new/choose) for
  reproducible incorrect behavior.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for the build,
test, release, and CI standards every change must meet.

Good places to start:

- **New analyses** — `crates/unstave-core` (see the [crate table](#crates)).
- **Rendering** — `crates/unstave-report` (terminal, JSON, DOT, Mermaid, HTML).
- **Safe rewrites** — `crates/unstave-codemod`.
- **Test fixtures** — `tests/fixtures/`; adding a fixture for a new scenario is a
  great first contribution.

If a change is non-trivial, open an issue first so we can agree on the shape
before you write code.

## License

MIT
