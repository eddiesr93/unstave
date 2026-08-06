//! A package without its own `tsconfig.json` inherits the nearest ancestor's.
//!
//! This layout is extremely common: an app directory has a `package.json` so the
//! workspace tool picks it up, but shares the repo's root `tsconfig.json` for path
//! aliases. Excalidraw is built this way — `excalidraw-app/` has a manifest and no
//! tsconfig, while the root declares every `@excalidraw/*` alias.
//!
//! Looking only inside the package directory produced a resolver with no `paths` at
//! all, so every aliased import failed silently. The failure mode is the dangerous
//! one: not an error, just a graph missing most of its edges, and therefore every
//! downstream count understated rather than reported missing.

use std::path::{Path, PathBuf};

use unstave_core::pipeline::analyze;
use unstave_core::{Config, Resolved};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/inherited-tsconfig")
}

#[test]
fn a_package_without_a_tsconfig_inherits_the_root_one() {
    let analysis = analyze(&fixture(), &Config::default()).expect("analysis should not fail");

    let app = analysis
        .modules
        .iter()
        .find(|m| m.path().ends_with("app/src/main.ts"))
        .expect("app main.ts should be discovered");

    let resolved = app
        .resolutions
        .get("@fixture/lib")
        .expect("the specifier should be recorded");

    assert!(
        matches!(resolved, Resolved::Internal { path } if path.ends_with("packages/lib/src/index.ts")),
        "the alias is declared only in the root tsconfig, so resolving it proves the \
         lookup walked upwards; got {resolved:?}"
    );
    assert!(
        analysis.unresolved.is_empty(),
        "nothing should be left unresolved: {:?}",
        analysis.unresolved
    );
}

#[test]
fn the_app_package_is_pointed_at_the_inherited_tsconfig() {
    let analysis = analyze(&fixture(), &Config::default()).expect("analysis should not fail");

    let app = analysis
        .workspace
        .packages
        .iter()
        .find(|p| p.name.as_deref() == Some("@fixture/app"))
        .expect("app package should be discovered");

    let tsconfig = app
        .tsconfig
        .as_ref()
        .expect("the app should inherit a tsconfig rather than having none");
    assert!(
        tsconfig.starts_with(&analysis.workspace.root) && tsconfig.ends_with("tsconfig.json"),
        "expected the root tsconfig, got {tsconfig:?}"
    );
}

/// The search must stop at the workspace root rather than escaping into whatever
/// happens to sit above it on the developer's machine.
#[test]
fn the_search_does_not_escape_the_workspace_root() {
    let root = fixture().join("packages");
    let analysis = analyze(&root, &Config::default()).expect("analysis should not fail");

    let lib = analysis
        .workspace
        .packages
        .iter()
        .find(|p| p.name.as_deref() == Some("@fixture/lib"))
        .expect("lib package should be discovered");

    // Analysed from `packages/`, the root tsconfig one level up is out of scope.
    assert!(
        lib.tsconfig.is_none(),
        "lookup should stop at the analysed root, got {:?}",
        lib.tsconfig
    );
}
