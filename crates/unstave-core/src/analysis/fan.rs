use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::analysis::reach::Reachability;
use crate::graph::ModuleGraph;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FanEntry {
    pub path: PathBuf,
    /// Modules importing this one, directly.
    pub direct: usize,
    /// Modules reaching this one through any number of hops.
    pub transitive: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FanReport {
    /// Most depended-upon modules — changing these ripples furthest.
    pub fan_in: Vec<FanEntry>,
    /// Modules pulling in the most others — the expensive entry points.
    pub fan_out: Vec<FanEntry>,
}

/// Top `limit` modules by transitive dependents and dependencies.
pub fn compute(graph: &ModuleGraph, include_type_edges: bool, limit: usize) -> FanReport {
    let forward = Reachability::forward(graph, include_type_edges);
    let reverse = Reachability::reverse(graph, include_type_edges);

    let mut fan_in: Vec<FanEntry> = Vec::with_capacity(graph.node_count());
    let mut fan_out: Vec<FanEntry> = Vec::with_capacity(graph.node_count());

    for node in graph.node_indices() {
        let path = graph.path_of(node).to_path_buf();
        fan_in.push(FanEntry {
            path: path.clone(),
            direct: graph.predecessors(node, include_type_edges).len(),
            transitive: reverse.count(node),
        });
        fan_out.push(FanEntry {
            path,
            direct: graph.successors(node, include_type_edges).len(),
            transitive: forward.count(node),
        });
    }

    // Ties broken by path so the report is stable run to run.
    let sort = |entries: &mut Vec<FanEntry>| {
        entries.sort_by(|a, b| {
            b.transitive
                .cmp(&a.transitive)
                .then_with(|| b.direct.cmp(&a.direct))
                .then_with(|| a.path.cmp(&b.path))
        });
        entries.retain(|e| e.transitive > 0 || e.direct > 0);
        entries.truncate(limit);
    };

    sort(&mut fan_in);
    sort(&mut fan_out);

    FanReport { fan_in, fan_out }
}
