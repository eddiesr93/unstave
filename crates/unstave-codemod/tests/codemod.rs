use std::path::{Path, PathBuf};

use unstave_codemod::{plan, CodemodOptions, SkipReason};
use unstave_core::config::ImportStyle;
use unstave_core::graph::ModuleGraph;
use unstave_core::{analyze, Config};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

#[test]
fn rewrites_one_named_barrel_import_and_preserves_every_other_byte() {
    let root = fixture("pure-barrel");
    let analysis = analyze(&root, &Config::default()).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);
    let options = CodemodOptions {
        import_style: ImportStyle::Relative,
        ..CodemodOptions::default()
    };

    let result =
        plan(&analysis, &graph, &Config::default(), &options).expect("codemod should plan");

    assert_eq!(result.files_changed(), 1);
    assert_eq!(result.imports_rewritten, 1);
    let change = &result.files[0];
    assert!(change.path.ends_with("src/main.ts"));
    assert_eq!(
        change.original,
        "import { AlphaClient } from '@/clients';\n\nexport const client = new AlphaClient();\n"
    );
    assert_eq!(
        change.rewritten,
        "import { AlphaClient } from './clients/alpha';\n\nexport const client = new AlphaClient();\n"
    );
    // The import span is the only touched region: the suffix is byte-identical.
    let original_suffix = change.original.split_once(";\n").expect("import suffix").1;
    let rewritten_suffix = change.rewritten.split_once(";\n").expect("import suffix").1;
    assert_eq!(original_suffix.as_bytes(), rewritten_suffix.as_bytes());
}

#[test]
fn alias_style_uses_the_shortest_matching_tsconfig_path() {
    let root = fixture("pure-barrel");
    let config = Config::default();
    let analysis = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);
    let options = CodemodOptions {
        import_style: ImportStyle::Alias,
        ..CodemodOptions::default()
    };

    let result = plan(&analysis, &graph, &config, &options).expect("codemod should plan");

    assert_eq!(result.files_changed(), 1);
    assert_eq!(
        result.files[0].rewritten.lines().next(),
        Some("import { AlphaClient } from '@/clients/alpha';")
    );
}

#[test]
fn preserve_style_follows_the_importers_predominant_path_style() {
    let root = fixture("pure-barrel");
    let config = Config::default();
    let analysis = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);

    let result =
        plan(&analysis, &graph, &config, &CodemodOptions::default()).expect("codemod should plan");

    assert_eq!(
        result.files[0].rewritten.lines().next(),
        Some("import { AlphaClient } from '@/clients/alpha';")
    );
}

#[test]
fn preserves_default_symbol_aliases_and_type_modifiers_when_splitting() {
    let root = fixture("codemod");
    let config = Config::default();
    let analysis = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);
    let options = CodemodOptions {
        import_style: ImportStyle::Relative,
        ..CodemodOptions::default()
    };

    let result = plan(&analysis, &graph, &config, &options).expect("codemod should plan");

    assert_eq!(result.files_changed(), 1);
    assert_eq!(result.imports_rewritten, 1);
    assert_eq!(
        result.files[0].rewritten,
        "// keep this exact header\n\
import type { User, Role as LocalRole } from './types';\n\
import DefaultWidget, { Widget as LocalWidget } from './widget';\n\
\n\
export const values: [typeof DefaultWidget, LocalWidget, User, LocalRole] | null = null;\n"
    );
    let (original_header, original_body) = result.files[0]
        .original
        .split_once("import DefaultWidget")
        .expect("original import");
    let (rewritten_header, rewritten_imports) = result.files[0]
        .rewritten
        .split_once("import type")
        .expect("rewritten imports");
    assert_eq!(original_header.as_bytes(), rewritten_header.as_bytes());
    let original_body = original_body
        .split_once(";\n")
        .expect("original import terminator")
        .1;
    let rewritten_body = rewritten_imports
        .split_once("./widget';\n")
        .expect("last rewritten import")
        .1;
    assert_eq!(original_body.as_bytes(), rewritten_body.as_bytes());
    insta::assert_snapshot!(
        "split_default_named_and_type_imports",
        result.files[0].rewritten
    );
}

#[test]
fn follows_reexport_alias_chains_to_the_original_symbol() {
    let root = fixture("aliases");
    let config = Config::default();
    let analysis = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);
    let options = CodemodOptions {
        import_style: ImportStyle::Relative,
        ..CodemodOptions::default()
    };

    let result = plan(&analysis, &graph, &config, &options).expect("codemod should plan");

    assert_eq!(result.files_changed(), 1);
    assert_eq!(
        result.files[0].rewritten,
        "import { Original as FinalName } from './inner/impl';\nexport const used = FinalName;\n"
    );
}

#[test]
fn merges_rewritten_bindings_into_an_existing_direct_import() {
    let root = fixture("codemod-merge");
    let config = Config::default();
    let analysis = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);
    let options = CodemodOptions {
        import_style: ImportStyle::Relative,
        ..CodemodOptions::default()
    };

    let result = plan(&analysis, &graph, &config, &options).expect("codemod should plan");

    assert_eq!(result.files_changed(), 1);
    assert_eq!(
        result.files[0].rewritten,
        "// direct import stays in this position\n\
import { helper, Widget as LocalWidget } from './widget';\n\
\n\
\n\
export const values = [helper, LocalWidget];\n"
    );
    assert_eq!(
        result.files[0].rewritten.matches("from './widget'").count(),
        1,
        "the target module must have one merged import"
    );
    let original_suffix = result.files[0]
        .original
        .split_once("from './index';\n")
        .expect("barrel import")
        .1;
    let rewritten_suffix = result.files[0]
        .rewritten
        .split_once("from './widget';\n\n")
        .expect("merged import")
        .1;
    assert_eq!(original_suffix.as_bytes(), rewritten_suffix.as_bytes());
    insta::assert_snapshot!("merged_existing_direct_import", result.files[0].rewritten);
}

#[test]
fn skips_a_barrel_with_observed_side_effects() {
    let root = fixture("side-effects");
    let config = Config::default();
    let analysis = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);

    let result =
        plan(&analysis, &graph, &config, &CodemodOptions::default()).expect("codemod should plan");

    assert_eq!(result.files_changed(), 0);
    assert_eq!(result.imports_rewritten, 0);
    assert!(result
        .skipped
        .iter()
        .any(|skip| skip.reason == SkipReason::BarrelHasSideEffects && skip.imports == 1));
}

#[test]
fn skips_when_an_existing_namespace_import_cannot_accept_named_bindings() {
    let root = fixture("codemod-merge-conflict");
    let config = Config::default();
    let analysis = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);

    let result =
        plan(&analysis, &graph, &config, &CodemodOptions::default()).expect("codemod should plan");

    assert_eq!(result.files_changed(), 0);
    assert!(result
        .skipped
        .iter()
        .any(|skip| skip.reason == SkipReason::MergeConflict && skip.imports == 1));
}

#[test]
fn only_glob_limits_which_importing_files_are_rewritten() {
    let root = fixture("pure-barrel");
    let config = Config::default();
    let analysis = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);
    let options = CodemodOptions {
        only: Some("src/not-main.ts".to_string()),
        ..CodemodOptions::default()
    };

    let result = plan(&analysis, &graph, &config, &options).expect("codemod should plan");

    assert_eq!(result.files_changed(), 0);
    assert_eq!(result.imports_rewritten, 0);
}

#[test]
fn barrel_scope_limits_which_barrel_is_unwound() {
    let root = fixture("pure-barrel");
    let config = Config::default();
    let analysis = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);
    let options = CodemodOptions {
        barrel: Some(PathBuf::from("src/not-the-barrel.ts")),
        ..CodemodOptions::default()
    };

    let result = plan(&analysis, &graph, &config, &options).expect("codemod should plan");

    assert_eq!(result.files_changed(), 0);
}

#[test]
fn namespace_barrel_imports_are_reported_and_left_untouched() {
    let root = fixture("codemod-merge-conflict");
    let config = Config::default();
    let analysis = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);

    let result =
        plan(&analysis, &graph, &config, &CodemodOptions::default()).expect("codemod should plan");

    assert!(result
        .skipped
        .iter()
        .any(|skip| skip.reason == SkipReason::NamespaceImport && skip.imports == 1));
}

#[test]
fn one_ambiguous_symbol_skips_the_whole_import_statement() {
    let root = fixture("star-collision");
    let mut config = Config::default();
    config.barrel.reexport_ratio = 0.6;
    let analysis = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);

    let result =
        plan(&analysis, &graph, &config, &CodemodOptions::default()).expect("codemod should plan");

    assert_eq!(result.files_changed(), 0);
    assert!(result
        .skipped
        .iter()
        .any(|skip| skip.reason == SkipReason::Ambiguous && skip.imports == 1));
}

#[test]
fn dry_run_exposes_a_standard_unified_diff() {
    let root = fixture("pure-barrel");
    let config = Config::default();
    let analysis = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);
    let options = CodemodOptions {
        import_style: ImportStyle::Relative,
        ..CodemodOptions::default()
    };
    let result = plan(&analysis, &graph, &config, &options).expect("codemod should plan");

    let diff = result.unified_diff(&analysis.workspace.root);

    assert!(diff.contains("--- a/src/main.ts\n+++ b/src/main.ts"));
    assert!(diff.contains("-import { AlphaClient } from '@/clients';"));
    assert!(diff.contains("+import { AlphaClient } from './clients/alpha';"));
}
