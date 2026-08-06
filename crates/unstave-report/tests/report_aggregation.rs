//! Focused assertions on `build_report`'s aggregation of the graph-level analyses.
//!
//! The full JSON snapshots in `report_renderers.rs` already lock the complete report
//! shape. These tests assert the *meaningful* numbers behind that shape for a named
//! fixture — barrel amplification (`maxAmplification`/`totalExcess`), cycle ordering,
//! and dead-export low-confidence propagation — so a regression in the aggregation
//! logic cannot hide behind a blanket snapshot update.

use std::path::{Path, PathBuf};

use unstave_core::graph::ModuleGraph;
use unstave_core::{analyze, Config};
use unstave_report::{build_report, AnalysisReport};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

/// Analyze a fixture and build the full report, zeroing wall-clock timings so the
/// output is deterministic (same as `report_renderers.rs` / `terminal_renderer.rs`).
fn report(name: &str) -> AnalysisReport {
    let root = fixture(name);
    let config = Config::default();
    let analysis = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);
    let mut report = build_report(&analysis, &graph, &config, false);
    report.timings.discovery_ms = 0;
    report.timings.parse_ms = 0;
    report.timings.resolve_ms = 0;
    report.timings.total_ms = 0;
    report
}

#[test]
fn barrel_amplification_reports_exact_values_for_pure_barrel() {
    let report = report("pure-barrel");

    // `src/main.ts` imports only `AlphaClient` from the barrel, but the barrel
    // re-exports five clients (`alpha`, `beta`, `gamma`, `delta`, `epsilon`), so
    // pulling it in costs 6 modules (the barrel plus all five clients). The minimal
    // cost — just the definition site of `AlphaClient` — is 1. Hence:
    //   excess        = 6 - 1  = 5
    //   amplification = 6 / 1  = 6.0
    assert_eq!(report.amplification.barrels.len(), 1);
    let barrel = &report.amplification.barrels[0];

    assert_eq!(barrel.barrel, "src/clients/index.ts");
    assert_eq!(barrel.import_sites, 1);
    assert_eq!(barrel.actual_cost, 6);
    assert_eq!(barrel.worst_excess, 5);
    assert_eq!(barrel.total_excess, 5);
    assert_eq!(barrel.max_amplification, 6.0);
    assert_eq!(barrel.rewritable_symbols, 1);
    assert_eq!(barrel.skipped_symbols, 0);
    assert!(!barrel.has_side_effects);
}

#[test]
fn barrel_amplification_max_equals_max_over_import_sites() {
    // The `nested-barrels` fixture has three classified barrels but only the outer
    // one is imported. `src/main.ts` imports `one` through the `a` → `a/b` → `a/b/c`
    // chain, which reaches 6 modules (the three barrels plus `one`, `two`, `three`)
    // while the minimal cost is just `one.ts`. So this barrel has one site with
    // actual cost 6, minimal cost 1, excess 5 and amplification 6.0.
    let report = report("nested-barrels");
    let sites = &report.amplification.sites;
    assert_eq!(sites.len(), 1);

    let site = &sites[0];
    assert_eq!(site.barrel, "src/a/index.ts");
    assert_eq!(site.actual_cost, 6);
    assert_eq!(site.minimal_cost, 1);
    assert_eq!(site.excess, 5);
    assert_eq!(site.amplification, 6.0);

    // The aggregate must be the exact max over every import site, never a guess.
    let barrels = &report.amplification.barrels;
    assert_eq!(barrels.len(), 1);
    let max_over_sites = sites
        .iter()
        .map(|site| site.amplification)
        .fold(0.0_f64, f64::max);
    assert_eq!(barrels[0].max_amplification, max_over_sites);
    // `totalExcess` is the sum of every site's excess, so it dominates each of them.
    assert!(barrels[0].total_excess >= barrels[0].worst_excess);
    assert_eq!(barrels[0].total_excess, 5);
}

#[test]
fn cycles_are_ordered_by_size_and_members_are_sorted() {
    let report = report("cycles");
    assert_eq!(report.cycles.len(), 2);

    // The `cycles` fixture has two SCCs: `a`↔`b` and `w`→`x`→{`y`,`z`}→`w`. The
    // 4-module component is reported first because the biggest components come
    // first, and members are sorted within each cycle.
    let first = &report.cycles[0];
    assert_eq!(
        first.members,
        vec!["src/w.ts", "src/x.ts", "src/y.ts", "src/z.ts"]
    );
    assert_eq!(
        first.shortest_path,
        vec!["src/w.ts", "src/x.ts", "src/z.ts", "src/w.ts"]
    );

    let second = &report.cycles[1];
    assert_eq!(second.members, vec!["src/a.ts", "src/b.ts"]);
    assert_eq!(
        second.shortest_path,
        vec!["src/a.ts", "src/b.ts", "src/a.ts"]
    );

    // Ordering is deterministic: bigger components first, members sorted.
    assert!(
        report.cycles[0].members.len() > report.cycles[1].members.len(),
        "largest cycle should be reported first"
    );
    for cycle in &report.cycles {
        let mut sorted = cycle.members.clone();
        sorted.sort();
        assert_eq!(cycle.members, sorted, "cycle members should be sorted");
    }
}

#[test]
fn dead_export_low_confidence_propagates_for_star_exports() {
    // `star-collision` mixes `export *` barrels with direct imports. Any export
    // reachable only through a star re-export is `low_confidence`, because the AST
    // cannot see the property access that may use it; exports declared or imported
    // directly are confident.
    let report = report("star-collision");
    let by = |module: &str, name: &str| {
        report
            .dead_exports
            .iter()
            .find(|export| export.module == module && export.name == name)
            .unwrap_or_else(|| panic!("expected dead export `{module}` -> `{name}`"))
    };

    // Reached through `export *` in `index.ts` / `shadowed.ts`: low confidence.
    assert!(by("src/left.ts", "shared").low_confidence);
    assert!(by("src/right.ts", "onlyRight").low_confidence);
    assert!(by("src/right.ts", "shared").low_confidence);

    // Declared/imported directly: the analysis is confident.
    assert!(!by("src/main.ts", "all").low_confidence);
    assert!(!by("src/shadowed.ts", "shared").low_confidence);
}

#[test]
fn bare_barrel_imports_cost_exactly_what_they_need() {
    // `src/main.ts` reaches the barrel twice without naming a symbol: a side-effect
    // `import './widgets/index'` and a dynamic `import('./widgets/index')`. Neither
    // can be rewritten, so the barrel's whole closure (4 modules: the barrel plus
    // three widgets) is the minimal cost as well as the actual one — zero excess and
    // an amplification of exactly 1.0. Treating the definition set as empty here used
    // to make both sites divide by zero and report the closure as removable excess.
    let report = report("bare-barrel-import");

    let sites = &report.amplification.sites;
    assert_eq!(sites.len(), 2);
    for site in sites {
        assert_eq!(site.barrel, "src/widgets/index.ts");
        assert!(site.symbols.is_empty());
        assert_eq!(site.actual_cost, 4);
        assert_eq!(site.minimal_cost, 4);
        assert_eq!(site.excess, 0);
        assert_eq!(site.amplification, 1.0);
    }

    assert_eq!(report.amplification.barrels.len(), 1);
    let barrel = &report.amplification.barrels[0];
    assert_eq!(barrel.import_sites, 2);
    assert_eq!(barrel.total_excess, 0);
    assert_eq!(barrel.worst_excess, 0);
    assert_eq!(barrel.max_amplification, 1.0);
}

/// §7 makes the JSON the versioned, CI-consumable artifact, so every amplification
/// value in it has to be a number a consumer can compare against a threshold.
/// `serde_json` writes a non-finite `f64` as `null`, which silently makes
/// `site.amplification > threshold` false instead of true.
#[test]
fn json_amplification_values_are_never_null() {
    for name in [
        "bare-barrel-import",
        "pure-barrel",
        "nested-barrels",
        "star-collision",
        "side-effects",
        "monorepo",
        "cycles",
    ] {
        let json = serde_json::to_value(report(name)).expect("report should serialize");
        let amplification = &json["amplification"];

        for site in amplification["sites"].as_array().expect("sites array") {
            assert!(
                site["amplification"].as_f64().is_some_and(f64::is_finite),
                "{name}: site amplification is not a finite number: {site}"
            );
        }
        for barrel in amplification["barrels"].as_array().expect("barrels array") {
            assert!(
                barrel["maxAmplification"]
                    .as_f64()
                    .is_some_and(f64::is_finite),
                "{name}: barrel maxAmplification is not a finite number: {barrel}"
            );
        }
    }
}
