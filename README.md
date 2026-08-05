# unstave

Module graph analyzer and barrel codemod for TypeScript/React monorepos.

`unstave` builds the full module graph of your workspace, finds where barrel files
(`index.ts` re-export hubs) are amplifying import cost, and rewrites those imports
to point at the modules that actually declare the symbols.

The problem it exists to solve: a single `import { Foo } from '@/clients'` can pull
hundreds of modules into your dev server's graph, because the barrel re-exports all
of them. `unstave` measures that amplification precisely, then fixes it.

> **Status: early.** Under active development against a written spec. Not yet
> published to crates.io or npm. Interfaces will change.

## What it is not

Not a bundler, not a linter. It does no TypeScript type checking — no `tsc`, no type
inference — and no bundle-size analysis.

## Install

Not yet published. For now:

```bash
cargo install --path crates/unstave-cli
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
unstave fix [--barrel <path>] [--only <glob>] [--write|--check]
```

Global flags: `--config <path>`, `--root <path>`, `--no-cache`, `-v/-vv`.

Non-terminal formats are written as `unstave-report.json`, `.dot`, `.mmd`, and
`.html`. The default directory is `<root>/.unstave`; `--out` overrides it. JSON is
complete and versioned with `schemaVersion: 1`. The HTML report is a single portable
file with no CDN or runtime network dependency. DOT and Mermaid group modules by
directory and collapse large graphs to at most `--max-nodes` directory nodes
(default: 150).

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

## Crates

| Crate | Purpose |
|---|---|
| `unstave-core` | Discovery, parsing, graph construction, analyses. A library — no I/O policy, no printing. |
| `unstave-report` | Terminal, JSON, DOT, Mermaid and HTML renderers. |
| `unstave-cli` | The `unstave` binary. |

## Development

```bash
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all
```

Built on [oxc](https://oxc.rs) for parsing and resolution. The `oxc_*` crates release
in lockstep, so they are pinned to a single exact version across the workspace —
mixing versions breaks the AST.

## License

MIT
