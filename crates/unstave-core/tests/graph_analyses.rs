use std::path::{Path, PathBuf};

use unstave_core::analysis::{cycles, fan, reach::Reachability};
use unstave_core::graph::{EdgeKind, ModuleGraph};
use unstave_core::pipeline::analyze;
use unstave_core::Config;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

struct Built {
    graph: ModuleGraph,
    root: PathBuf,
}

fn build(name: &str) -> Built {
    let root = fixture(name);
    let analysis = analyze(&root, &Config::default()).expect("analysis should not fail");
    Built {
        graph: ModuleGraph::build(&analysis.modules),
        root: analysis.workspace.root.clone(),
    }
}

impl Built {
    fn node(&self, rel: &str) -> petgraph::graph::NodeIndex {
        self.graph
            .index_of(&self.root.join(rel))
            .unwrap_or_else(|| panic!("{rel} should be a graph node"))
    }

    fn rel(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }
}

#[test]
fn builds_nodes_and_classifies_edges() {
    let built = build("pure-barrel");
    assert_eq!(built.graph.node_count(), 7);

    // The barrel's five re-exports plus main's one static import.
    let kinds = built.graph.edge_kind_counts();
    assert_eq!(
        kinds,
        vec![(EdgeKind::Static, 1), (EdgeKind::ReExport, 5)]
            .into_iter()
            .collect::<Vec<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    );
}

#[test]
fn type_only_edges_are_excluded_from_runtime_traversal_by_default() {
    let built = build("type-only");
    let main = built.node("src/main.ts");

    // main imports ./types twice — once `import type`, once mixed — and ./helper.
    // The mixed import is a runtime edge, so ./types stays reachable either way.
    let runtime = built.graph.successors(main, false);
    let with_types = built.graph.successors(main, true);
    assert_eq!(runtime.len(), 2);
    assert_eq!(with_types.len(), 2);

    // Both edge kinds exist between main and types.
    let kinds = built.graph.edge_kind_counts();
    assert!(kinds.iter().any(|(k, _)| *k == EdgeKind::TypeOnly));
    assert!(kinds.iter().any(|(k, _)| *k == EdgeKind::Static));
}

#[test]
fn finds_both_cycles_and_leaves_acyclic_modules_alone() {
    let built = build("cycles");
    let found = cycles::find(&built.graph, false);

    assert_eq!(
        found.len(),
        2,
        "expected the 2-node cycle and the 4-node SCC"
    );

    // Largest first.
    let big = &found[0];
    assert_eq!(big.size(), 4);
    let mut members: Vec<String> = big.members.iter().map(|p| built.rel(p)).collect();
    members.sort();
    assert_eq!(
        members,
        vec!["src/w.ts", "src/x.ts", "src/y.ts", "src/z.ts"]
    );

    let small = &found[1];
    assert_eq!(small.size(), 2);
    let mut members: Vec<String> = small.members.iter().map(|p| built.rel(p)).collect();
    members.sort();
    assert_eq!(members, vec!["src/a.ts", "src/b.ts"]);

    // standalone.ts is in no cycle.
    assert!(found
        .iter()
        .flat_map(|c| &c.members)
        .all(|p| !built.rel(p).contains("standalone")));
}

#[test]
fn reports_a_closed_shortest_path_per_cycle() {
    let built = build("cycles");
    let found = cycles::find(&built.graph, false);

    for cycle in &found {
        let path: Vec<String> = cycle.shortest_path.iter().map(|p| built.rel(p)).collect();
        assert!(
            path.len() >= 3,
            "a cycle path needs at least start -> other -> start, got {path:?}"
        );
        assert_eq!(
            path.first(),
            path.last(),
            "the path must be closed, got {path:?}"
        );
        // Every step must be a real edge.
        for pair in cycle.shortest_path.windows(2) {
            let from = built.graph.index_of(&pair[0]).expect("node");
            let to = built.graph.index_of(&pair[1]).expect("node");
            assert!(
                built.graph.successors(from, false).contains(&to),
                "{:?} -> {:?} is not an edge",
                built.rel(&pair[0]),
                built.rel(&pair[1])
            );
        }
    }

    // The 4-node SCC has a chord x -> z, so the shortest cycle through w is
    // w -> x -> z -> w (4 entries closed), not the full 5-entry ring.
    let big = &found[0];
    let path: Vec<String> = big.shortest_path.iter().map(|p| built.rel(p)).collect();
    assert_eq!(
        path,
        vec!["src/w.ts", "src/x.ts", "src/z.ts", "src/w.ts"],
        "BFS should find the chord, not walk the whole ring"
    );
}

#[test]
fn transitive_reachability_counts_the_barrel_closure() {
    let built = build("pure-barrel");
    let forward = Reachability::forward(&built.graph, false);

    // The barrel reaches all five clients.
    let barrel = built.node("src/clients/index.ts");
    assert_eq!(forward.count(barrel), 5);

    // main reaches the barrel plus everything behind it — the amplification, though
    // it only ever asked for one symbol.
    let main = built.node("src/main.ts");
    assert_eq!(forward.count(main), 6);

    // A leaf reaches nothing.
    let alpha = built.node("src/clients/alpha.ts");
    assert_eq!(forward.count(alpha), 0);

    // The union primitive M4 needs: importing only alpha would cost 1 module.
    assert_eq!(forward.union_count(&[alpha]), 1);
}

#[test]
fn cycle_members_all_reach_each_other() {
    let built = build("cycles");
    let forward = Reachability::forward(&built.graph, false);

    // Every member of the 4-node SCC reaches all four, including itself.
    for name in ["src/w.ts", "src/x.ts", "src/y.ts", "src/z.ts"] {
        assert_eq!(
            forward.count(built.node(name)),
            4,
            "{name} should reach all four cycle members"
        );
    }

    let standalone = built.node("src/standalone.ts");
    assert_eq!(forward.count(standalone), 0);
}

#[test]
fn ranks_fan_in_and_fan_out() {
    let built = build("pure-barrel");
    let report = fan::compute(&built.graph, false, 10);

    // main pulls in the most.
    let top_out = &report.fan_out[0];
    assert_eq!(built.rel(&top_out.path), "src/main.ts");
    assert_eq!(top_out.transitive, 6);
    assert_eq!(top_out.direct, 1);

    // Each client is reached by both the barrel and main.
    let alpha = report
        .fan_in
        .iter()
        .find(|e| built.rel(&e.path) == "src/clients/alpha.ts")
        .expect("alpha should appear in fan-in");
    assert_eq!(alpha.direct, 1, "only the barrel imports alpha directly");
    assert_eq!(alpha.transitive, 2, "the barrel and main both reach it");
}

/// Regression: the native binding's `includeTypeEdges` test builds its assertion on
/// fan-in membership for a type-only re-export chain. If the graph ever dropped the
/// type-only edges into a type-only module (e.g. because a resolved path's separators
/// stopped matching the canonicalised discovery path on Windows), the module would
/// disappear from fan-in. Guard that the edges — and therefore the fan-in — behave the
/// same here as they do on every platform.
#[test]
fn type_only_reexport_module_enters_fan_in_only_with_type_edges() {
    let built = build("type-reexport");

    let in_fan = |report: &fan::FanReport| {
        report
            .fan_in
            .iter()
            .any(|e| built.rel(&e.path) == "src/clients/models/ThingDto.ts")
    };

    let without_type = fan::compute(&built.graph, false, usize::MAX);
    assert!(
        !in_fan(&without_type),
        "type-only module should be absent from fan-in when type edges are excluded"
    );

    let with_type = fan::compute(&built.graph, true, usize::MAX);
    let entry = with_type
        .fan_in
        .iter()
        .find(|e| built.rel(&e.path) == "src/clients/models/ThingDto.ts")
        .expect("type-only module should appear in fan-in when type edges are included");
    assert_eq!(
        entry.direct, 2,
        "both the barrel and load import ThingDto directly"
    );
    // main -> barrel -> ThingDto, plus load's direct import.
    assert!(entry.transitive > 1, "main and the barrel both reach it");
}

/// The SCC-condensation reachability is a performance optimization over per-node BFS,
/// and its ordering requirements are subtle enough to have been wrong twice. Check it
/// against the obvious implementation on every fixture, in both directions.
#[test]
fn reachability_agrees_with_naive_bfs() {
    for name in ["simple", "pure-barrel", "cycles", "type-only", "monorepo"] {
        let built = build(name);
        for include_type_edges in [false, true] {
            let forward = Reachability::forward(&built.graph, include_type_edges);
            let reverse = Reachability::reverse(&built.graph, include_type_edges);

            for node in built.graph.node_indices() {
                let expect_fwd = naive_reachable(&built.graph, node, include_type_edges, true);
                assert_eq!(
                    forward.count(node),
                    expect_fwd.len(),
                    "forward mismatch in {name} (type edges: {include_type_edges}) at {}",
                    built.rel(built.graph.path_of(node))
                );

                let expect_rev = naive_reachable(&built.graph, node, include_type_edges, false);
                assert_eq!(
                    reverse.count(node),
                    expect_rev.len(),
                    "reverse mismatch in {name} (type edges: {include_type_edges}) at {}",
                    built.rel(built.graph.path_of(node))
                );
            }
        }
    }
}

/// Plain BFS, deliberately naive — this is the oracle, so it must be obviously right.
fn naive_reachable(
    graph: &ModuleGraph,
    start: petgraph::graph::NodeIndex,
    include_type_edges: bool,
    forward: bool,
) -> std::collections::HashSet<petgraph::graph::NodeIndex> {
    let mut seen = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();

    let step = |n: petgraph::graph::NodeIndex| {
        if forward {
            graph.successors(n, include_type_edges)
        } else {
            graph.predecessors(n, include_type_edges)
        }
    };

    for next in step(start) {
        if seen.insert(next) {
            queue.push_back(next);
        }
    }
    while let Some(node) = queue.pop_front() {
        for next in step(node) {
            if seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    seen
}

#[test]
fn records_dangling_edges_when_a_target_is_excluded() {
    let root = fixture("simple");
    let config = Config {
        exclude: vec!["**/math.ts".to_string()],
        ..Config::default()
    };
    let analysis = analyze(&root, &config).expect("analysis should not fail");
    let graph = ModuleGraph::build(&analysis.modules);

    // math.ts resolved but was excluded from discovery, so the edge has no target.
    assert_eq!(graph.dangling().len(), 1);
    let (from, to) = &graph.dangling()[0];
    assert!(from.ends_with("main.ts"));
    assert!(to.ends_with("math.ts"));
    assert_eq!(graph.node_count(), 2);
}
