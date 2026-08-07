# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-08-07

### Fixed

- Preserve explicit `.js`, `.mjs`, and `.cjs` runtime extensions when rewriting
  TypeScript imports to their direct source definitions. This keeps NodeNext and
  ESM imports runnable instead of replacing their runtime specifiers with source
  extensions.
- Preserve each source file's semicolon convention in generated imports so the
  codemod produces formatter-clean changes in semicolon-free codebases.

### Added

- Publish reproducible validation against pinned Vite, TanStack Query, and Astro
  revisions. The rewritten code passed Vite's package build and typecheck, 168
  TanStack Query tests, and 84 targeted Astro tests.
- Add a professional GitHub Pages landing page, structured documentation, sample
  report, discovery metadata, contribution paths, and community templates.

## [0.2.0] - 2026-08-06

### Changed

- **Breaking (JSON schema):** `schemaVersion` is now `2`. `amplification.sites[*].amplification`
  and `amplification.barrels[*].maxAmplification` are always finite numbers and are
  never `null`. They previously serialized as `null` whenever the metric was infinite,
  so a CI consumer writing `b.maxAmplification > threshold` silently got `false`
  instead of a breach. When an import's symbols all resolve outside the workspace the
  minimal cost is zero, and the ratio is now taken against a floor of one module
  rather than reported as an infinity JSON cannot represent.

### Fixed

- Stop reporting a barrel's entire closure as removable excess for an import that
  names no symbols. `import './barrel'` and `import('./barrel')` carry no bindings,
  so nothing can be rewritten and the whole barrel is genuinely required — but their
  minimal cost was computed from an empty definition set, making the excess equal to
  the full actual cost and the amplification infinite. Such a site now costs exactly
  what it needs: zero excess, amplification 1.0, matching how namespace imports are
  already treated. On `withastro/astro`, this drops `src/core/config/index.ts` from a
  reported 167 total excess to 65 and takes `dev-toolbar/ui-library/index.ts` — which
  had 13 excess against 0 rewritable symbols — down to 0.

## [0.1.5] - 2026-08-06

### Fixed

- Make module resolution path matching cross-platform (Windows). The resolver now
  classifies a resolved path as internal vs. external by comparing it against the
  workspace root in the same canonical key form the graph uses for its node index,
  instead of a byte- and case-sensitive `Path::starts_with`. A path that discovery
  would spell differently on Windows (back/forward slashes, the `\\?\` prefix, drive
  or component case, or uncollapsed `..`) is no longer misclassified as external,
  which previously could silently drop every edge into it.
- Satisfy the clippy `io_other_error` lint in the core error test.

### CI & tooling

- Add Windows native test coverage to CI so the native bindings are exercised on
  Windows, not just the platform where CI previously ran.

## [0.1.4] - 2026-08-06

### Fixed

- Make the native binding tests cross-platform so they pass on Windows. The graph
  node index is now keyed by a separator-normalised module path, so graph edges
  (and the fan-in/fan-out built from them) are identical on every platform even
  when the resolver emits paths whose separators differ from the canonical
  discovery form.

## [0.1.3] - 2026-08-06

### Fixed

- Resolve aliased local re-exports to their real definition in the core
  symbol/barrel analysis.

### Changed

- Replace the `DashMap` memo in `SymbolResolver` with a single-threaded map for
  better resolution performance.
- Factor closure-walking into a shared graph primitive in `unstave-core`.
- Unify `SkipReason` into a single core enum.
- Remove the dead legacy terminal rendering path in `unstave-report`.
- Derive `@unstave/node` public types from the generated native types and commit
  the native loader.

### Added

- Test coverage for the terminal renderer, `build_report` aggregation, native
  binding behavior, codemod multi-file/merge/tsx/idempotency scenarios, and core
  error paths (parse failures, bad config, missing root).

### CI & tooling

- Add JavaScript linting (oxlint) and dependency/security scanning (pnpm audit,
  cargo audit, gitleaks, dependency review) to CI.
- Validate musl native targets in CI.
- Pin GitHub Actions to commit SHAs.
- Resolve a dangling benchmark budget spec reference in the docs.

## [0.1.2] - 2026-08-06

Initial public release of the analyzers, CLI, NAPI bindings, and Vite plugin.

- Fix the HTML report on large graphs, the entrypoint projection, and the
  warm-run cost.

[Unreleased]: https://github.com/eddiesr93/unstave/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/eddiesr93/unstave/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/eddiesr93/unstave/compare/v0.1.5...v0.2.0
[0.1.5]: https://github.com/eddiesr93/unstave/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/eddiesr93/unstave/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/eddiesr93/unstave/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/eddiesr93/unstave/releases/tag/v0.1.2
