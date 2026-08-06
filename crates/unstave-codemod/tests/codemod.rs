use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use unstave_codemod::{plan, CodemodOptions, CodemodPlan, SkipReason};
use unstave_core::config::ImportStyle;
use unstave_core::graph::ModuleGraph;
use unstave_core::{analyze, Config};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

static SCRATCH_ID: AtomicU64 = AtomicU64::new(0);

/// Copy a fixture into a fresh scratch directory so a plan can be applied to disk
/// without mutating the shared fixtures on disk.
fn scratch_fixture(name: &str) -> PathBuf {
    let unique = format!(
        "unstave-codemod-test-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos(),
        SCRATCH_ID.fetch_add(1, Ordering::Relaxed)
    );
    let target = std::env::temp_dir().join(unique);
    copy_tree(&fixture(name), &target);
    target
}

fn copy_tree(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).expect("create scratch fixture");
    for entry in std::fs::read_dir(source).expect("read fixture") {
        let entry = entry.expect("fixture entry");
        let destination = target.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            std::fs::copy(entry.path(), destination).expect("copy fixture file");
        }
    }
}

/// Write every planned rewrite back to disk, modelling a real apply pass.
fn apply_plan_to_disk(plan: &CodemodPlan) {
    for change in &plan.files {
        std::fs::write(&change.path, &change.rewritten).expect("write rewritten source");
    }
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
fn rewrites_aliased_local_reexport_to_the_defining_module() {
    // `src/index.ts` re-exports the imported `foo` under a new local alias:
    // `import { foo } from './impl'; export { foo as bar };`. A consumer importing
    // `bar` must be re-pointed at `./impl`, not left pointed at the barrel.
    let root = fixture("local-alias");
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
        "import { foo as bar } from './impl';\n\nexport const used = bar;\n"
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

#[test]
fn rewrites_barrel_imports_across_multiple_files_in_one_plan() {
    let root = fixture("multi-file");
    let config = Config::default();
    let analysis = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);
    let options = CodemodOptions {
        import_style: ImportStyle::Relative,
        ..CodemodOptions::default()
    };

    let result = plan(&analysis, &graph, &config, &options).expect("codemod should plan");

    assert_eq!(result.files_changed(), 2);
    assert_eq!(result.imports_rewritten, 2);
    let a = result
        .files
        .iter()
        .find(|change| change.path.ends_with("a.ts"))
        .expect("a.ts should be planned");
    let b = result
        .files
        .iter()
        .find(|change| change.path.ends_with("b.ts"))
        .expect("b.ts should be planned");
    assert_eq!(
        a.rewritten,
        "import { Widget } from './widget';\n\nexport const a = new Widget();\n"
    );
    assert_eq!(
        b.rewritten,
        "import { Widget } from './widget';\n\nexport const b = new Widget();\n"
    );
    // Only the import span changes in each file: the suffix is byte-identical.
    for change in [a, b] {
        let original_suffix = change.original.split_once(";\n").expect("import suffix").1;
        let rewritten_suffix = change.rewritten.split_once(";\n").expect("import suffix").1;
        assert_eq!(original_suffix.as_bytes(), rewritten_suffix.as_bytes());
    }
}

#[test]
fn merges_rewritten_bindings_into_an_existing_import_without_duplicating() {
    // `main.ts` already imports `Widget` directly from `./widget`; the barrel
    // import also binds `Widget`. After merging, `Widget` must appear once.
    let root = fixture("codemod-merge-dup");
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
        "import { Widget, helper } from './widget';\n\n\nexport const values = [Widget, helper];\n"
    );
    let import_line = result.files[0]
        .rewritten
        .lines()
        .next()
        .expect("import line");
    assert_eq!(
        import_line.matches("Widget").count(),
        1,
        "the existing binding must be merged, not duplicated"
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
}

#[test]
fn rewrites_a_barrel_import_in_a_tsx_file_and_preserves_every_other_byte() {
    let root = fixture("tsx");
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
    let change = &result.files[0];
    assert!(change.path.ends_with("main.tsx"));
    assert_eq!(
        change.original,
        "import { Widget } from './index';\n\nexport const app = <Widget />;\n"
    );
    assert_eq!(
        change.rewritten,
        "import { Widget } from './widget';\n\nexport const app = <Widget />;\n"
    );
    // The JSX body after the import is byte-identical.
    let original_suffix = change.original.split_once(";\n").expect("import suffix").1;
    let rewritten_suffix = change.rewritten.split_once(";\n").expect("import suffix").1;
    assert_eq!(original_suffix.as_bytes(), rewritten_suffix.as_bytes());
}

#[test]
fn rewrites_a_mid_file_barrel_import_without_touching_the_surrounding_code() {
    // The barrel import sits below other statements, not at the top of the file.
    let root = fixture("mid-file");
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
        "import { helper } from './helper';\n\n\
         export function setup() {\n  return helper();\n}\n\n\
         import { Widget } from './widget';\n\n\
         export const widget = new Widget();\n"
    );
    // Everything before and after the rewritten import is byte-identical.
    let (original_before, original_tail) = result.files[0]
        .original
        .split_once("import { Widget } from './index';")
        .expect("barrel import");
    let (rewritten_before, rewritten_tail) = result.files[0]
        .rewritten
        .split_once("import { Widget } from './widget';")
        .expect("rewritten import");
    assert_eq!(original_before.as_bytes(), rewritten_before.as_bytes());
    assert_eq!(original_tail.as_bytes(), rewritten_tail.as_bytes());
}

#[test]
fn applying_the_plan_to_an_already_rewritten_file_yields_no_further_changes() {
    let root = scratch_fixture("pure-barrel");
    let config = Config::default();
    let options = CodemodOptions {
        import_style: ImportStyle::Relative,
        ..CodemodOptions::default()
    };

    let first = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&first.modules);
    let first_plan = plan(&first, &graph, &config, &options).expect("codemod should plan");
    assert!(first_plan.files_changed() > 0);
    apply_plan_to_disk(&first_plan);

    let second = analyze(&root, &config).expect("rewritten fixture should analyze");
    let graph = ModuleGraph::build(&second.modules);
    let second_plan = plan(&second, &graph, &config, &options).expect("codemod should plan again");

    assert_eq!(second_plan.files_changed(), 0);
    assert_eq!(second_plan.imports_rewritten, 0);
}

#[test]
fn passing_the_same_source_through_twice_is_stable() {
    // The second pass is a no-op: re-applying a plan to already-rewritten output
    // leaves every file byte-for-byte unchanged.
    let root = scratch_fixture("pure-barrel");
    let config = Config::default();
    let options = CodemodOptions {
        import_style: ImportStyle::Relative,
        ..CodemodOptions::default()
    };

    let first = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&first.modules);
    let first_plan = plan(&first, &graph, &config, &options).expect("codemod should plan");
    assert!(first_plan.files_changed() > 0);
    apply_plan_to_disk(&first_plan);

    let second = analyze(&root, &config).expect("rewritten fixture should analyze");
    let graph = ModuleGraph::build(&second.modules);
    let second_plan = plan(&second, &graph, &config, &options).expect("codemod should plan again");
    assert_eq!(second_plan.files_changed(), 0);
    // No rewrite is applied on the second pass, so the on-disk file is unchanged.
    apply_plan_to_disk(&second_plan);
    let final_source = std::fs::read_to_string(root.join("src/main.ts")).expect("read file");
    assert!(final_source.contains("from './clients/alpha'"));
}
