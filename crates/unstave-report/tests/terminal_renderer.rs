use std::path::{Path, PathBuf};

use unstave_core::analysis::symbols::SymbolResolver;
use unstave_core::analysis::{amplification, barrel, cycles};
use unstave_core::graph::ModuleGraph;
use unstave_core::{analyze, Analysis, Config};
use unstave_report::{build_report, terminal, AnalysisReport, RenderOptions};

/// The same fixtures exercised by `report_renderers.rs`. `pure-barrel` drives the
/// barrel amplification paths, `cycles` drives the cycle tree, and the rest are
/// snapshot over the full terminal report.
const FIXTURES: &[&str] = &[
    "simple",
    "unresolved",
    "monorepo",
    "pure-barrel",
    "type-only",
    "cycles",
    "star-collision",
    "aliases",
    "nested-barrels",
    "side-effects",
];

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn opts() -> RenderOptions {
    RenderOptions {
        color: false,
        max_rows: 0,
    }
}

/// Analyze a fixture and build the full report, zeroing wall-clock timings so the
/// output is deterministic snapshot material (same as `report_renderers.rs`).
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

/// Re-run the graph-level analyses so the focused renderers (`render_barrels`,
/// `render_cycles`) get their raw inputs, exactly as the CLI subcommands do.
fn loaded(name: &str) -> (Analysis, ModuleGraph, Config) {
    let root = fixture(name);
    let config = Config::default();
    let analysis = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);
    (analysis, graph, config)
}

fn amplification_report(
    analysis: &Analysis,
    graph: &ModuleGraph,
    config: &Config,
) -> amplification::AmplificationReport {
    let symbols = SymbolResolver::new(graph, &analysis.modules);
    let barrels = barrel::classify(graph, &config.barrel);
    let entrypoints = config.entrypoint_paths(&analysis.workspace.root);
    amplification::compute(
        graph,
        &analysis.modules,
        &barrels,
        &symbols,
        &entrypoints,
        false,
    )
}

#[test]
fn render_report_snapshots_every_fixture() {
    for name in FIXTURES {
        let rendered = terminal::render_report(&report(name), &opts());
        insta::assert_snapshot!(format!("report_{name}"), rendered);
    }
}

#[test]
fn render_report_reports_summary_counts() {
    let rendered = terminal::render_report(&report("simple"), &opts());
    // The summary table carries the headline counts from the fixture.
    assert!(rendered.contains("files analyzed"));
    assert!(rendered.contains("modules"));
    assert!(rendered.contains("edges"));
    assert!(rendered.contains("3"));
    assert!(rendered.contains("2"));
}

#[test]
fn render_report_renders_no_cycles_when_there_are_none() {
    let rendered = terminal::render_report(&report("simple"), &opts());
    assert!(rendered.contains("No cycles."));
    // No cycle tree is printed for an acyclic graph.
    assert!(!rendered.contains("  modules:"));
}

#[test]
fn render_cycles_lists_every_cycle_member() {
    let (analysis, graph, _config) = loaded("cycles");
    let found = cycles::find(&graph, false);
    let rendered = terminal::render_cycles(&analysis, &found, &opts());

    assert!(rendered.contains("2 cycle(s) covering 6 module(s)"));
    // Each of the cycle members appears in the closed-path tree. `y.ts` is a member
    // of the 4-module cycle but not on its `shortest_path`, so only the tree members
    // are asserted here.
    for member in ["src/a.ts", "src/b.ts", "src/w.ts", "src/x.ts", "src/z.ts"] {
        assert!(
            rendered.contains(member),
            "cycle member `{member}` should be listed"
        );
    }
    assert!(rendered.contains("┌─"));
    assert!(rendered.contains("└─"));
    // standalone.ts is not part of any cycle.
    assert!(!rendered.contains("src/standalone.ts"));
}

#[test]
fn render_cycles_snapshots_the_cycle_tree() {
    let (analysis, graph, _config) = loaded("cycles");
    let found = cycles::find(&graph, false);
    let rendered = terminal::render_cycles(&analysis, &found, &opts());
    insta::assert_snapshot!("cycles", rendered);
}

#[test]
fn render_cycles_reports_none_when_graph_is_acyclic() {
    let (analysis, graph, _config) = loaded("pure-barrel");
    let found = cycles::find(&graph, false);
    let rendered = terminal::render_cycles(&analysis, &found, &opts());
    assert_eq!(rendered.trim(), "No cycles.");
}

#[test]
fn render_dead_exports_lists_dead_export_names() {
    let rendered = terminal::render_dead_exports(&report("pure-barrel").dead_exports, &opts(), 0);
    // Every dead export reported by the analysis appears by name.
    for name in [
        "BetaClient",
        "DeltaClient",
        "EpsilonClient",
        "GammaClient",
        "client",
    ] {
        assert!(
            rendered.contains(name),
            "dead export `{name}` should be mentioned"
        );
    }
    assert!(rendered.contains("Dead exports"));
    assert!(rendered.contains("high"));
}

#[test]
fn render_dead_exports_reports_none_when_all_exports_are_live() {
    let empty: Vec<unstave_report::report::DeadExportReport> = Vec::new();
    let rendered = terminal::render_dead_exports(&empty, &opts(), 0);
    assert_eq!(rendered.trim(), "No dead exports found.");
}

#[test]
fn render_barrels_shows_barrel_path_and_amplification() {
    let (analysis, graph, config) = loaded("pure-barrel");
    let amp = amplification_report(&analysis, &graph, &config);
    let rendered = terminal::render_barrels(&analysis, &amp, None, &opts());

    // The barrel path (workspace-relative) and its amplification ratio are both shown.
    assert!(rendered.contains("src/clients/index.ts"));
    assert!(rendered.contains("6.0×"));
    assert!(rendered.contains("1/1"));
    assert!(rendered.contains("Barrel amplification"));
    assert!(rendered.contains("1 barrel(s) classified, 1 of them imported"));
}

#[test]
fn render_barrels_snapshots_the_amplification_table() {
    let (analysis, graph, config) = loaded("pure-barrel");
    let amp = amplification_report(&analysis, &graph, &config);
    let rendered = terminal::render_barrels(&analysis, &amp, None, &opts());
    insta::assert_snapshot!("barrels", rendered);
}

#[test]
fn render_barrels_reports_nothing_when_no_barrel_is_imported() {
    let (analysis, graph, config) = loaded("simple");
    let amp = amplification_report(&analysis, &graph, &config);
    let rendered = terminal::render_barrels(&analysis, &amp, None, &opts());
    assert!(rendered.contains("No barrel imports to report"));
}
