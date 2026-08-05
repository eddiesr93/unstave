use std::path::PathBuf;

use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};

use crate::config::BarrelConfig;
use crate::graph::ModuleGraph;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BarrelKind {
    /// Nothing is declared here; the module exists only to forward.
    Pure,
    /// Mostly forwarding, but it declares a few things of its own.
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Barrel {
    pub path: PathBuf,
    pub kind: BarrelKind,
    pub export_count: usize,
    pub reexport_count: usize,
    pub own_decl_count: usize,
    /// Dropping an import of this barrel could drop a side effect, so the codemod
    /// must leave it alone and report it for manual review.
    pub has_side_effects: bool,
    #[serde(skip)]
    pub node: NodeIndex,
}

impl Barrel {
    pub fn reexport_ratio(&self) -> f64 {
        if self.export_count == 0 {
            0.0
        } else {
            self.reexport_count as f64 / self.export_count as f64
        }
    }
}

/// Classify every module against the barrel thresholds.
pub fn classify(graph: &ModuleGraph, config: &BarrelConfig) -> Vec<Barrel> {
    let mut barrels: Vec<Barrel> = graph
        .node_indices()
        .filter_map(|node| classify_one(graph, node, config))
        .collect();
    barrels.sort_by(|a, b| a.path.cmp(&b.path));
    barrels
}

fn classify_one(graph: &ModuleGraph, node: NodeIndex, config: &BarrelConfig) -> Option<Barrel> {
    let facts = &graph.node(node).facts;

    let export_count = facts.exports.len();
    if export_count == 0 {
        return None;
    }
    let reexport_count = facts.exports.iter().filter(|e| e.is_reexport()).count();

    let ratio = reexport_count as f64 / export_count as f64;
    if ratio < config.reexport_ratio || facts.own_decl_count > config.max_own_decls {
        return None;
    }

    Some(Barrel {
        path: facts.path.clone(),
        kind: if facts.own_decl_count == 0 {
            BarrelKind::Pure
        } else {
            BarrelKind::Mixed
        },
        export_count,
        reexport_count,
        own_decl_count: facts.own_decl_count,
        has_side_effects: facts.has_side_effects,
        node,
    })
}
