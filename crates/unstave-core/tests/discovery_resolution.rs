use std::path::{Path, PathBuf};

use unstave_core::pipeline::{analyze, relative};
use unstave_core::{Config, Resolved, WorkspaceKind};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn run(name: &str) -> unstave_core::Analysis {
    let root = fixture(name);
    analyze(&root, &Config::default()).expect("analysis should not fail")
}

/// Workspace-relative, slash-separated paths so assertions read the same on any OS.
fn rel_files(analysis: &unstave_core::Analysis) -> Vec<String> {
    let mut files: Vec<String> = analysis
        .modules
        .iter()
        .map(|m| {
            relative(&analysis.workspace.root, m.path())
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    files.sort();
    files
}

#[test]
fn discovers_source_files_and_skips_manifests() {
    let analysis = run("simple");
    assert_eq!(analysis.workspace.kind, WorkspaceKind::Single);
    assert_eq!(
        rel_files(&analysis),
        vec!["src/greet.ts", "src/main.ts", "src/math.ts"]
    );
    // tsconfig.json and package.json are inputs to resolution, not modules.
    assert!(analysis.parse_failures().is_empty());
}

#[test]
fn resolves_relative_alias_and_builtin_specifiers() {
    let analysis = run("simple");
    let main = analysis
        .modules
        .iter()
        .find(|m| m.path().ends_with("main.ts"))
        .expect("main.ts should be discovered");

    // Relative import.
    assert!(matches!(
        main.resolutions.get("./math"),
        Some(Resolved::Internal { path }) if path.ends_with("math.ts")
    ));

    // tsconfig `paths` alias — this is the one that matters for real repos.
    assert!(
        matches!(
            main.resolutions.get("@/greet"),
            Some(Resolved::Internal { path }) if path.ends_with("greet.ts")
        ),
        "expected @/greet to resolve via tsconfig paths, got {:?}",
        main.resolutions.get("@/greet")
    );

    // `node:` protocol.
    assert!(matches!(
        main.resolutions.get("node:fs/promises"),
        Some(Resolved::Builtin { name }) if name == "fs/promises"
    ));

    assert!(analysis.unresolved.is_empty(), "{:?}", analysis.unresolved);
}

#[test]
fn records_unresolved_specifiers_without_failing() {
    let analysis = run("unresolved");

    let mut specs: Vec<&str> = analysis
        .unresolved
        .iter()
        .map(|u| u.specifier.as_str())
        .collect();
    specs.sort_unstable();
    assert_eq!(specs, vec!["./does-not-exist", "some-uninstalled-package"]);

    // Every miss points back at a real importer, with a span and a reason.
    for u in &analysis.unresolved {
        assert!(u.importer.ends_with("main.ts"));
        assert!(!u.reason.is_empty());
        assert!(u.span.end > u.span.start, "span should cover the statement");
    }

    // The resolvable import in the same file still resolves.
    let main = &analysis.modules[0];
    assert!(matches!(
        main.resolutions.get("./real"),
        Some(Resolved::Internal { .. })
    ));
}

#[test]
fn detects_workspace_packages_and_cross_package_imports() {
    let analysis = run("monorepo");

    assert_eq!(analysis.workspace.kind, WorkspaceKind::Pnpm);

    let mut names: Vec<&str> = analysis
        .workspace
        .packages
        .iter()
        .filter_map(|p| p.name.as_deref())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec![
            "@fixture/ui",
            "@fixture/utils",
            "@fixture/web",
            "fixture-monorepo"
        ]
    );

    // `sideEffects: false` is read off package.json.
    let utils = analysis
        .workspace
        .packages
        .iter()
        .find(|p| p.name.as_deref() == Some("@fixture/utils"))
        .expect("utils package");
    assert!(utils.side_effects_false);

    let ui = analysis
        .workspace
        .packages
        .iter()
        .find(|p| p.name.as_deref() == Some("@fixture/ui"))
        .expect("ui package");
    assert_eq!(ui.public_entrypoints.len(), 1);
    assert!(ui.public_entrypoints[0].ends_with("packages/ui/src/index.ts"));

    // A cross-package import through node_modules resolves back into the workspace,
    // so it must be Internal — not External — or the graph would stop at the boundary.
    let web = analysis
        .modules
        .iter()
        .find(|m| m.path().ends_with("apps/web/src/main.ts"))
        .expect("web main.ts");
    assert!(
        matches!(
            web.resolutions.get("@fixture/ui"),
            Some(Resolved::Internal { path }) if path.ends_with("packages/ui/src/index.ts")
        ),
        "expected @fixture/ui to resolve into the workspace, got {:?}",
        web.resolutions.get("@fixture/ui")
    );
}

#[test]
fn applies_per_package_tsconfig_paths() {
    let analysis = run("monorepo");
    let ui = analysis
        .modules
        .iter()
        .find(|m| m.path().ends_with("packages/ui/src/index.ts"))
        .expect("ui index.ts");

    // `~/*` is defined only in packages/ui/tsconfig.json, so resolving it proves
    // each package gets its own resolver rather than sharing the root's.
    assert!(
        matches!(
            ui.resolutions.get("~/theme"),
            Some(Resolved::Internal { path }) if path.ends_with("theme.ts")
        ),
        "expected ~/theme to resolve via the ui package tsconfig, got {:?}",
        ui.resolutions.get("~/theme")
    );
}

#[test]
fn excludes_are_honoured() {
    let root = fixture("simple");
    let config = Config {
        exclude: vec!["**/math.ts".to_string()],
        ..Config::default()
    };
    let analysis = analyze(&root, &config).expect("analysis should not fail");

    assert_eq!(rel_files(&analysis), vec!["src/greet.ts", "src/main.ts"]);
    // The excluded file is gone from the graph, but the import pointing at it still
    // resolves — dangling targets are a graph concern, handled at M3.
    let main = analysis
        .modules
        .iter()
        .find(|m| m.path().ends_with("main.ts"))
        .expect("main.ts");
    assert!(matches!(
        main.resolutions.get("./math"),
        Some(Resolved::Internal { .. })
    ));
}
