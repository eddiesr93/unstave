# unstave

Module graph analyzer and barrel codemod for TypeScript/React monorepos.

`unstave` builds the full module graph of your workspace, finds where barrel files
(`index.ts` re-export hubs) are amplifying import cost, and rewrites those imports
to point at the modules that actually declare the symbols.

The problem it exists to solve: a single `import { Foo } from '@/clients'` can pull
hundreds of modules into your dev server's graph, because the barrel re-exports all
of them. `unstave` measures that amplification precisely, then fixes it.

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

The native Node API and Vite integration are published separately:

```bash
npm install unstave
npm install --save-dev vite-plugin-unstave
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
complete and versioned with `schemaVersion: 1`. The HTML report is a single portable
file with no CDN or runtime network dependency. DOT and Mermaid group modules by
directory and collapse large graphs to at most `--max-nodes` directory nodes
(default: 150).

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
```

## Vite plugin

`vite-plugin-unstave` runs the native analysis asynchronously, so Vite startup and
HMR do not wait for the module graph. In development it serves the live report at
`/__unstave`; production analysis is opt-in and writes JSON plus HTML at `buildEnd`.

```ts
import { defineConfig } from 'vite'
import unstave from 'vite-plugin-unstave'

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

The release benchmark generates 6000 TypeScript modules and enforces the cold
and warm budgets from the specification:

```bash
cargo bench -p unstave-core --bench pipeline
```

See [docs/benchmarks.md](docs/benchmarks.md) for the latest measured results and
machine details.

Built on [oxc](https://oxc.rs) for parsing and resolution. The `oxc_*` crates release
in lockstep, so they are pinned to a single exact version across the workspace —
mixing versions breaks the AST.

## License

MIT
