use std::path::{Path, PathBuf};

use unstave_core::graph::ModuleGraph;
use unstave_core::{analyze, Config};
use unstave_report::{build_report, dot, html, json, mermaid};

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
    "bare-barrel-import",
];

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn report(name: &str) -> unstave_report::AnalysisReport {
    let root = fixture(name);
    let config = Config::default();
    let analysis = analyze(&root, &config).expect("fixture should analyze");
    let graph = ModuleGraph::build(&analysis.modules);
    let mut report = build_report(&analysis, &graph, &config, false);
    // Wall-clock timings are useful in real artifacts but not snapshot material.
    report.timings.discovery_ms = 0;
    report.timings.parse_ms = 0;
    report.timings.resolve_ms = 0;
    report.timings.total_ms = 0;
    report
}

#[test]
fn snapshots_complete_json_for_every_fixture() {
    for name in FIXTURES {
        let rendered = json::render(&report(name)).expect("JSON should serialize");
        insta::assert_snapshot!(format!("json_{name}"), rendered);
    }
}

#[test]
fn serialized_keys_are_all_camel_case() {
    let value = serde_json::to_value(report("side-effects")).expect("report serializes");
    assert_camel_case_keys(&value, "$");
}

fn assert_camel_case_keys(value: &serde_json::Value, at: &str) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                assert!(
                    is_camel_case(key),
                    "serialized key `{key}` at {at} is not camelCase"
                );
                assert_camel_case_keys(child, &format!("{at}.{key}"));
            }
        }
        serde_json::Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                assert_camel_case_keys(child, &format!("{at}[{index}]"));
            }
        }
        _ => {}
    }
}

fn is_camel_case(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_lowercase())
        && !key.contains('_')
        && !key.contains('-')
}

#[test]
fn report_paths_are_workspace_relative_and_forward_slashed() {
    let report = report("monorepo");
    assert_eq!(report.workspace.root, ".");
    assert!(report
        .modules
        .iter()
        .all(|module| !module.path.starts_with('/') && !module.path.contains('\\')));
    let rendered = json::render(&report).expect("JSON should serialize");
    assert!(!rendered.contains(env!("CARGO_MANIFEST_DIR")));
}

#[test]
fn graph_renderers_cluster_and_collapse_deterministically() {
    let report = report("nested-barrels");
    let full_dot = dot::render(&report, 150);
    assert!(full_dot.contains("subgraph cluster_"));
    assert!(full_dot.contains("label=\"re-export\""));
    assert!(!full_dot.contains("directory aggregates"));

    let collapsed_dot = dot::render(&report, 2);
    assert!(collapsed_dot.contains("directory aggregates"));
    let collapsed_mermaid = mermaid::render(&report, 2);
    assert!(collapsed_mermaid.contains("directory aggregates"));
    assert!(collapsed_mermaid.contains("classDef barrel"));
    assert_eq!(collapsed_dot, dot::render(&report, 2));
    assert_eq!(collapsed_mermaid, mermaid::render(&report, 2));
}

#[test]
fn html_is_self_contained_and_embeds_the_projected_graph() {
    let report = report("pure-barrel");
    let rendered = html::render(&report, html::DEFAULT_MAX_NODES).expect("HTML should serialize");
    assert!(rendered.starts_with("<!doctype html>"));
    assert!(rendered.contains("Cytoscape Consortium"));
    assert!(rendered.contains("id=\"unstave-data\""));
    assert!(rendered.contains("Barrel amplification"));
    assert!(rendered.contains("Graph filters"));
    assert!(rendered.contains("src/clients/index.ts"));
    assert!(!rendered.contains("<script src="));
    assert!(!rendered.contains("<link href="));
}

/// The page draws the projected graph, the summary and the barrel table. Embedding the
/// rest of the report is what made a 5,000-module workspace produce a 4 MB page that
/// pinned the layout engine, so the payload is asserted to stay narrow.
#[test]
fn html_embeds_only_what_the_page_draws() {
    let report = report("cycles");
    let rendered = html::render(&report, html::DEFAULT_MAX_NODES).expect("HTML should serialize");
    let payload = embedded_payload(&rendered);

    assert!(payload.get("graph").is_some());
    assert!(payload.get("summary").is_some());
    assert!(payload.pointer("/amplification/barrels").is_some());
    for absent in [
        "modules",
        "edges",
        "deadExports",
        "cycles",
        "fan",
        "timings",
    ] {
        assert!(
            payload.get(absent).is_none(),
            "{absent} is not drawn and should not be embedded"
        );
    }
}

#[test]
fn html_collapses_directories_past_the_node_budget() {
    let report = report("nested-barrels");
    let full = embedded_payload(&html::render(&report, 100).expect("HTML should serialize"));
    let collapsed = embedded_payload(&html::render(&report, 2).expect("HTML should serialize"));

    assert_eq!(full["graph"]["collapsed"], serde_json::Value::Bool(false));
    assert_eq!(
        collapsed["graph"]["collapsed"],
        serde_json::Value::Bool(true)
    );

    let nodes = collapsed["graph"]["nodes"].as_array().expect("nodes");
    assert!(
        nodes.len() <= 2,
        "collapsed graph kept {} nodes",
        nodes.len()
    );
    let counted: u64 = nodes
        .iter()
        .map(|node| node["moduleCount"].as_u64().unwrap_or(0))
        .sum();
    assert_eq!(
        counted, report.summary.modules as u64,
        "every module belongs to exactly one collapsed node"
    );
}

fn embedded_payload(rendered: &str) -> serde_json::Value {
    let start = rendered
        .find("id=\"unstave-data\" type=\"application/json\">")
        .map(|index| index + "id=\"unstave-data\" type=\"application/json\">".len())
        .expect("payload script element");
    let end = start
        + rendered[start..]
            .find("</script>")
            .expect("payload script close");
    serde_json::from_str(&rendered[start..end]).expect("payload should be valid JSON")
}
