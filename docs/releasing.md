# Releasing unstave

All publishable manifests must have the same version. Verify that invariant before
creating a release tag:

```bash
node scripts/check-release-version.mjs 0.1.3
cargo install --locked --path crates/unstave-cli --root /tmp/unstave-install
cargo package --workspace --locked --no-verify
pnpm build
pnpm test
```

Update the version and tag in `Formula/unstave.rb`, then push an annotated `vX.Y.Z`
tag. The release workflow:

1. validates the tag against every Cargo and npm manifest;
2. publishes the Rust crates in dependency order;
3. cross-compiles eight Node-API targets and tests the runnable bindings;
4. cross-compiles the standalone `unstave` CLI binary for seven platforms and
   stages it into the `@unstave/cli` platform packages;
5. publishes the platform npm packages, the `@unstave/node` loader,
   `@unstave/vite-plugin`, the `@unstave/cli` platform packages and loader —
   all with npm provenance.

The `@unstave/cli` loader keeps `optionalDependencies` out of the manifest until
publish time: `scripts/inject-cli-optional-dependencies.mjs` rewrites them into
`packages/cli/package.json` during the publish job, mirroring what napi does for
`@unstave/node`. Regenerate the committed platform package manifests after a
version bump with:

```bash
node scripts/create-cli-platform-packages.mjs
```

The workflow needs `CARGO_REGISTRY_TOKEN` and `NPM_TOKEN` repository secrets. The
GitHub-provided token is used for npm provenance and release metadata.
