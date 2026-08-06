# Contributing to unstave

Thanks for taking an interest in `unstave`. This guide covers the practical
mechanics of building, testing, and releasing the project so you can contribute
without guessing. For a broader overview of what the tool does and how it is
used, read [README.md](README.md) first.

## Repository layout

`unstave` is a Rust/TypeScript monorepo. The Rust workspace (see the crate
table in [README.md](README.md#crates)) holds the analysis engine, renderers,
codemod, Node-API binding, and CLI; the `packages/` directory holds the
JavaScript packages (`@unstave/node` and `@unstave/vite-plugin`).

| Path | Contents |
|---|---|
| `crates/unstave-core` | Discovery, parsing, graph construction, analyses. A library. |
| `crates/unstave-report` | Terminal, JSON, DOT, Mermaid and HTML renderers. |
| `crates/unstave-codemod` | Span-based, byte-preserving import-rewrite plans. |
| `crates/unstave-napi` | The Node-API boundary for reports and HTML. |
| `crates/unstave-cli` | The `unstave` binary. |
| `packages/unstave` | `@unstave/node` — the native binding and its loader. |
| `packages/vite-plugin-unstave` | `@unstave/vite-plugin` — the Vite plugin. |
| `tests/fixtures/` | Shared fixture workspaces used by both Rust and JS tests. |
| `docs/` | `benchmarks.md` (performance budgets) and `releasing.md`. |

## Building and testing

### Rust

```bash
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all
```

To run the release performance benchmark that enforces the acceptance budgets
(see [docs/benchmarks.md](docs/benchmarks.md)):

```bash
cargo bench -p unstave-core --bench pipeline
```

### JavaScript

The JavaScript packages are managed with pnpm:

```bash
pnpm install
pnpm build
pnpm test
```

`pnpm build` and `pnpm test` recurse through the workspace packages
(`pnpm --recursive ...`). `pnpm lint` runs oxlint, and `pnpm typecheck` runs the
per-package type checks. The native binding needs a build before its tests can
run, so run `pnpm build` at least once after a fresh install.

## Performance benchmark requirement

The 6000-module budget is enforced in CI. `cargo bench -p unstave-core --bench
pipeline` generates a fresh workspace of 6000 TypeScript modules and exits
non-zero if either budget is exceeded:

- Cold, cache miss: under 1500 ms
- Warm, content-hash cache hit: under 200 ms

It also asserts the exact module count and verifies the warm run was a real
cache hit. See [docs/benchmarks.md](docs/benchmarks.md) for the latest measured
results and machine details. If you change parsing, graph construction, or
caching, make sure this benchmark still passes.

## Code style

- **Rust** is formatted with `rustfmt` (see `rustfmt.toml`) and linted with
  `cargo clippy --all-targets --all-features -- -D warnings`. CI runs
  `cargo fmt --all -- --check` and clippy with `-D warnings` (and sets
  `RUSTFLAGS="-D warnings"` globally).
- **JavaScript** is linted with **oxlint** (`pnpm lint`). Configuration lives in
  `.oxlintrc.json`, which ignores `tests/fixtures/**`, `**/dist/**`, and
  `**/node_modules/**`.
- **Frontend / site** work follows the design standards in `.impeccable.md`.

CI runs all of these checks plus `cargo audit` for Rust dependencies, `pnpm
audit` for JavaScript dependencies, gitleaks for leaked secrets, and a
dependency review on pull requests. Make sure your branch is green before
requesting a review.

## Test fixtures

Both the Rust and JavaScript test suites run against the shared fixture
workspaces in `tests/fixtures/`. Each fixture is a self-contained mini-project
(a directory with its own `package.json` and `src/`) that exercises a specific
scenario — for example `simple`, `monorepo`, `cycles`, `side-effects`,
`type-only`, `tsx`, or `unresolved`.

### Adding a fixture

1. Create a new directory under `tests/fixtures/` named after the scenario
   (e.g. `tests/fixtures/my-scenario/`).
2. Give it a minimal `package.json` and the source files (`src/…`) needed to
   reproduce the scenario. Keep it as small as possible while still
   distinguishing the behavior you are testing.
3. Reference the fixture from the relevant test:
   - Rust tests in `crates/*/tests/` resolve fixtures relative to
     `tests/fixtures/`.
   - JS tests in `packages/*/test/` do the same.
4. Fixtures under `tests/fixtures/` are excluded from oxlint, so they do not
   need to satisfy the JS lint rules.

## Release process

Releases ship one version across **every** publishable manifest: the Cargo
workspace, the Homebrew formula, and each npm package (including the
per-platform native packages under `packages/unstave/npm/`). All of them must
carry the same version before a release tag is created.

Verify that invariant with:

```bash
node scripts/check-release-version.mjs <version>
```

The full procedure, including the commands run before tagging and what the
release workflow does, is documented in [docs/releasing.md](docs/releasing.md).
In short, a pushed `vX.Y.Z` tag:

1. validates the tag against every Cargo and npm manifest;
2. publishes the Rust crates in dependency order;
3. cross-compiles eight Node-API targets and tests the runnable bindings;
4. publishes the platform npm packages, the `@unstave/node` loader, and
   `@unstave/vite-plugin` with npm provenance.

The workflow needs `CARGO_REGISTRY_TOKEN` and `NPM_TOKEN` repository secrets.
Follow [docs/releasing.md](docs/releasing.md) when preparing a release.
