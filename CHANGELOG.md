# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/eddiesr93/unstave/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/eddiesr93/unstave/releases/tag/v0.1.2
