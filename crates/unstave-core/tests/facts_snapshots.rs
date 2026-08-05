//! Snapshot coverage for `ModuleFacts` extraction.
//!
//! Snapshots are taken over a normalized view: absolute paths become
//! workspace-relative with forward slashes, and content hashes are dropped, so the
//! snapshots are stable across machines and checkouts.

use std::path::{Path, PathBuf};

use serde::Serialize;
use unstave_core::pipeline::{analyze, relative};
use unstave_core::resolve::Resolved;
use unstave_core::{Config, ExportRecord, ImportRecord, ModuleFacts};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

/// A machine-independent projection of one module.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModuleView<'a> {
    path: String,
    has_side_effects: bool,
    package_side_effects_free: bool,
    own_decl_count: usize,
    imports: &'a [ImportRecord],
    exports: &'a [ExportRecord],
    /// Specifier → what it resolved to, paths normalized.
    resolutions: Vec<(String, String)>,
}

fn view<'a>(root: &Path, facts: &'a ModuleFacts, module: &unstave_core::Module) -> ModuleView<'a> {
    let resolutions = module
        .resolutions
        .iter()
        .map(|(spec, resolved)| {
            let rendered = match resolved {
                Resolved::Internal { path } => {
                    format!("internal:{}", norm(&relative(root, path)))
                }
                Resolved::External { package, .. } => format!("external:{package}"),
                Resolved::Builtin { name } => format!("builtin:{name}"),
                // Reasons carry OS-specific text, so only the kind is snapshotted.
                Resolved::Unresolved { .. } => "unresolved".to_string(),
            };
            (spec.clone(), rendered)
        })
        .collect();

    ModuleView {
        path: norm(&relative(root, &facts.path)),
        has_side_effects: facts.has_side_effects,
        package_side_effects_free: facts.package_side_effects_free,
        own_decl_count: facts.own_decl_count,
        imports: &facts.imports,
        exports: &facts.exports,
        resolutions,
    }
}

fn norm(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn snapshot_fixture(name: &str) -> Vec<String> {
    let root = fixture(name);
    let analysis = analyze(&root, &Config::default()).expect("analysis should not fail");
    assert!(
        analysis.parse_failures().is_empty(),
        "fixture should parse cleanly: {:?}",
        analysis.parse_failures()
    );

    let mut modules: Vec<_> = analysis
        .modules
        .iter()
        .map(|m| view(&analysis.workspace.root, &m.facts, m))
        .collect();
    modules.sort_by(|a, b| a.path.cmp(&b.path));

    modules
        .iter()
        .map(|m| serde_json::to_string_pretty(m).expect("view is serializable"))
        .collect()
}

#[test]
fn simple_fixture_facts() {
    insta::assert_snapshot!("simple", snapshot_fixture("simple").join("\n"));
}

#[test]
fn pure_barrel_fixture_facts() {
    insta::assert_snapshot!("pure-barrel", snapshot_fixture("pure-barrel").join("\n"));
}

#[test]
fn type_only_fixture_facts() {
    insta::assert_snapshot!("type-only", snapshot_fixture("type-only").join("\n"));
}

/// Type-only imports vanish under `verbatimModuleSyntax`, so they must be
/// distinguishable from runtime imports — statement-level and inline alike.
#[test]
fn type_only_imports_are_marked_at_both_levels() {
    let root = fixture("type-only");
    let analysis = analyze(&root, &Config::default()).expect("analysis should not fail");

    let main = analysis
        .modules
        .iter()
        .find(|m| m.path().ends_with("main.ts"))
        .expect("main module");

    // `import type { User }` — whole statement is type-only.
    let type_import = &main.facts.imports[0];
    assert!(type_import.type_only);
    assert!(type_import.is_type_only());

    // `import { DEFAULT_ROLE, type Role }` — mixed, so the statement survives even
    // though one binding does not.
    let mixed = &main.facts.imports[1];
    assert!(!mixed.type_only);
    assert!(!mixed.is_type_only(), "a mixed import is not type-only");
    assert_eq!(mixed.bindings.len(), 2);
    assert!(!mixed.bindings[0].type_only, "DEFAULT_ROLE is a value");
    assert!(mixed.bindings[1].type_only, "Role is a type");

    // `sideEffects: false` from package.json is recorded, separately from AST evidence.
    assert!(main.facts.package_side_effects_free);
    assert!(!main.facts.has_side_effects);
}

/// The serialized shape is a public artifact (the `--format json` report at M5 and
/// the napi boundary at M7), so every key must be camelCase. Mixed casing is easy to
/// introduce with `serde(tag = ...)` enums, where `rename_all` covers variant names
/// but not their fields.
#[test]
fn serialized_keys_are_all_camel_case() {
    let root = fixture("type-only");
    let analysis = analyze(&root, &Config::default()).expect("analysis should not fail");

    let json = serde_json::to_string(
        &analysis
            .modules
            .iter()
            .map(|m| &m.facts)
            .collect::<Vec<_>>(),
    )
    .expect("facts are serializable");

    let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    let mut offenders = Vec::new();
    collect_snake_case_keys(&value, &mut offenders);
    assert!(
        offenders.is_empty(),
        "these serialized keys are not camelCase: {offenders:?}"
    );
}

fn collect_snake_case_keys(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                if key.contains('_') {
                    out.push(key.clone());
                }
                collect_snake_case_keys(child, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_snake_case_keys(item, out);
            }
        }
        _ => {}
    }
}

/// The barrel shape the whole tool exists to detect: every export is a re-export,
/// nothing is declared locally, and one consumer pulls a single symbol through it.
#[test]
fn pure_barrel_has_the_expected_shape() {
    let root = fixture("pure-barrel");
    let analysis = analyze(&root, &Config::default()).expect("analysis should not fail");

    let barrel = analysis
        .modules
        .iter()
        .find(|m| m.path().ends_with("clients/index.ts"))
        .expect("barrel module");

    assert_eq!(
        barrel.facts.own_decl_count, 0,
        "a pure barrel declares nothing"
    );
    assert_eq!(barrel.facts.exports.len(), 5);
    assert!(barrel.facts.exports.iter().all(ExportRecord::is_reexport));
    assert!(!barrel.facts.has_side_effects);
    // All five re-export targets resolve inside the workspace, so the barrel's
    // transitive closure is the full set — that is the cost M4 will quantify.
    assert_eq!(barrel.internal_deps().len(), 5);

    let main = analysis
        .modules
        .iter()
        .find(|m| m.path().ends_with("src/main.ts"))
        .expect("main module");
    assert_eq!(main.facts.imports.len(), 1);
    assert_eq!(main.facts.imports[0].bindings.len(), 1);
    assert_eq!(main.facts.imports[0].bindings[0].imported, "AlphaClient");
    // One import, one symbol, but it lands on the barrel rather than alpha.ts.
    assert_eq!(main.internal_deps().len(), 1);
}
