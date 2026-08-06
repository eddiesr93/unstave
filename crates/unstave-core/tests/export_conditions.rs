//! Custom export conditions.
//!
//! Monorepos routinely point a custom condition at TypeScript source so that
//! development resolves to `src/` while published builds resolve to `dist/`.
//! TanStack Query does exactly this with `@tanstack/custom-condition`. Without the
//! condition, resolution lands on build output that may not exist yet, and every
//! cross-package import behind it silently fails to resolve — which makes barrel
//! amplification look far smaller than it is rather than obviously broken.

use std::path::{Path, PathBuf};

use unstave_core::config::ResolveConfig;
use unstave_core::pipeline::analyze;
use unstave_core::{Config, Resolved};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/export-conditions")
}

fn resolve_lib(conditions: &[&str]) -> Resolved {
    let config = Config {
        resolve: ResolveConfig {
            conditions: conditions.iter().map(|s| s.to_string()).collect(),
        },
        ..Config::default()
    };
    let analysis = analyze(&fixture(), &config).expect("analysis should not fail");
    let main = analysis
        .modules
        .iter()
        .find(|m| m.path().ends_with("app/src/main.ts"))
        .expect("app main.ts should be discovered");
    main.resolutions
        .get("@fixture/lib")
        .expect("the specifier should be recorded either way")
        .clone()
}

#[test]
fn without_the_condition_the_import_does_not_resolve() {
    // The `import`/`default` entries point at ./dist/index.js, which does not exist
    // in the fixture — exactly the state of an unbuilt workspace package.
    let resolved = resolve_lib(&[]);
    assert!(
        matches!(resolved, Resolved::Unresolved { .. }),
        "expected the unbuilt dist path to fail, got {resolved:?}"
    );
}

#[test]
fn the_condition_resolves_the_import_to_source() {
    let resolved = resolve_lib(&["@fixture/source"]);
    assert!(
        matches!(&resolved, Resolved::Internal { path } if path.ends_with("lib/src/index.ts")),
        "expected @fixture/source to resolve into the library source, got {resolved:?}"
    );
}

#[test]
fn an_unrelated_condition_changes_nothing() {
    let resolved = resolve_lib(&["some-other-condition"]);
    assert!(
        matches!(resolved, Resolved::Unresolved { .. }),
        "an unmatched condition must not accidentally resolve, got {resolved:?}"
    );
}

/// Custom conditions have to be tried *before* the defaults: `exports` maps are
/// matched in order, so a source-pointing condition listed after `import` would lose
/// to the build-output entry and never take effect.
#[test]
fn the_custom_condition_wins_over_the_default_entry() {
    let resolved = resolve_lib(&["@fixture/source"]);
    match resolved {
        Resolved::Internal { path } => {
            let text = path.to_string_lossy().replace('\\', "/");
            assert!(
                text.contains("/src/") && !text.contains("/dist/"),
                "condition order must favour source over dist, got {text}"
            );
        }
        other => panic!("expected an internal resolution, got {other:?}"),
    }
}
