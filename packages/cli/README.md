# @unstave/cli

The `unstave` command line tool as an npm package — **no Rust toolchain
required**. The native binary for your platform is installed automatically as an
optional dependency.

`unstave` maps your TypeScript/React module graph, ranks how much your `index.ts`
barrel files amplify your imports, exposes cycles and dead exports, and then
rewrites those imports to point at the actual declaration sites — byte-safely and
dry-run by default.

```bash
# Analyze the current workspace (terminal + interactive HTML report)
npx @unstave/cli analyze --format terminal --format html

# Rank every barrel by how much it amplifies your imports
npx @unstave/cli barrels

# Preview every proven-safe rewrite (touches nothing)
npx @unstave/cli fix --dry-run
```

Prefer to install it once:

```bash
npm install -g @unstave/cli
unstave analyze
```

> Prebuilt binaries are shipped for macOS (arm64/x64), Linux (glibc and musl,
> arm64/x64), and Windows (x64). For any other host — including Windows on
> Arm — install from source with `cargo install unstave-cli`.

## What it does

- **Measure** — rank barrels by amplification, actual reachable cost, and excess
  modules; export JSON, DOT, Mermaid, or a single-file interactive HTML report.
- **Find** — import cycles and dead exports across a monorepo.
- **Rewrite** — a span-based codemod that only changes import statements it can
  prove, preserving aliases, type modifiers, and byte-for-byte source elsewhere.

## Docs

- [Main repository](https://github.com/eddiesr93/unstave)
- [Documentation](https://eddiesr93.github.io/unstave/docs/)
- [Vite plugin](https://www.npmjs.com/package/@unstave/vite-plugin)

## Platform packages

This package installs one of the following optional binary packages at install
time:

| Package | Binary |
|---|---|
| `@unstave/cli-darwin-arm64` | aarch64-apple-darwin |
| `@unstave/cli-darwin-x64` | x86_64-apple-darwin |
| `@unstave/cli-linux-arm64-gnu` | aarch64 glibc |
| `@unstave/cli-linux-arm64-musl` | aarch64 musl |
| `@unstave/cli-linux-x64-gnu` | x86_64 glibc |
| `@unstave/cli-linux-x64-musl` | x86_64 musl |
| `@unstave/cli-win32-x64-msvc` | x86_64-pc-windows-msvc |

Windows on Arm (aarch64-pc-windows-msvc) does not yet ship a prebuilt binary;
the `unstave` command fails with a clear message pointing at
`cargo install unstave-cli`.

If the binary for your platform is missing, the `unstave` command fails with a
clear message and points you at `cargo install unstave-cli`.
