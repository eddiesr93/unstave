use std::collections::HashMap;
use std::path::{Path, PathBuf};

use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableDiGraph;
use petgraph::Direction;
use serde::{Deserialize, Serialize};

use crate::facts::{ImportKind, ModuleFacts};
use crate::pipeline::Module;
use crate::resolve::Resolved;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EdgeKind {
    /// A plain `import { x } from './a'`.
    Static,
    /// `import('./a')` — a separate chunk at build time, still a graph edge.
    Dynamic,
    /// Erased under `verbatimModuleSyntax`, so excluded from runtime cost by default.
    TypeOnly,
    /// `export { x } from './a'` — the edge that makes barrels expensive.
    ReExport,
    /// `import './a'` for effect only.
    SideEffectOnly,
}

impl EdgeKind {
    /// Whether this edge carries runtime cost. Type-only edges vanish in the emitted
    /// output, so they must not inflate any cost metric unless explicitly asked for.
    pub fn is_runtime(&self) -> bool {
        !matches!(self, EdgeKind::TypeOnly)
    }
}

#[derive(Debug, Clone)]
pub struct ModuleNode {
    pub path: PathBuf,
    pub facts: ModuleFacts,
}

impl ModuleNode {
    pub fn is_barrel_shaped(&self) -> bool {
        self.facts.exports.iter().any(|e| e.is_reexport())
    }
}

/// The internal module graph. External packages and builtins are deliberately absent:
/// they are leaves with no outgoing edges, so they add nodes without adding reachable
/// cost, and keeping them out keeps every traversal on internal modules only.
pub struct ModuleGraph {
    graph: StableDiGraph<ModuleNode, EdgeKind>,
    index: HashMap<PathBuf, NodeIndex>,
    /// Imports pointing at a path that resolved but was not discovered as a module —
    /// usually an `exclude` glob that hid the target.
    dangling: Vec<(PathBuf, PathBuf)>,
}

impl ModuleGraph {
    pub fn build(modules: &[Module]) -> Self {
        let mut graph = StableDiGraph::new();
        let mut index = HashMap::with_capacity(modules.len());

        for module in modules {
            let idx = graph.add_node(ModuleNode {
                path: module.facts.path.clone(),
                facts: module.facts.clone(),
            });
            index.insert(module.facts.path.clone(), idx);
        }

        let mut dangling = Vec::new();
        // Deduplicate (source, target, kind); one statement per line still produces one
        // edge per kind, which is what the renderers want to style.
        let mut seen = std::collections::HashSet::new();

        for module in modules {
            let Some(&from) = index.get(&module.facts.path) else {
                continue;
            };

            for (specifier, kind) in edge_kinds(&module.facts) {
                let Some(resolved) = module.resolutions.get(specifier) else {
                    continue;
                };
                let Resolved::Internal { path } = resolved else {
                    continue;
                };
                match index.get(path) {
                    Some(&to) => {
                        if seen.insert((from, to, kind)) {
                            graph.add_edge(from, to, kind);
                        }
                    }
                    None => dangling.push((module.facts.path.clone(), path.clone())),
                }
            }
        }

        dangling.sort();
        dangling.dedup();

        Self {
            graph,
            index,
            dangling,
        }
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    pub fn inner(&self) -> &StableDiGraph<ModuleNode, EdgeKind> {
        &self.graph
    }

    pub fn index_of(&self, path: &Path) -> Option<NodeIndex> {
        self.index.get(path).copied()
    }

    pub fn node(&self, idx: NodeIndex) -> &ModuleNode {
        &self.graph[idx]
    }

    pub fn path_of(&self, idx: NodeIndex) -> &Path {
        &self.graph[idx].path
    }

    pub fn node_indices(&self) -> impl Iterator<Item = NodeIndex> + '_ {
        self.graph.node_indices()
    }

    /// Imports whose target resolved but is not in the graph.
    pub fn dangling(&self) -> &[(PathBuf, PathBuf)] {
        &self.dangling
    }

    /// Direct successors along edges that pass `filter`.
    pub fn successors(&self, node: NodeIndex, include_type_edges: bool) -> Vec<NodeIndex> {
        self.neighbors(node, Direction::Outgoing, include_type_edges)
    }

    /// Direct predecessors along edges that pass `filter`.
    pub fn predecessors(&self, node: NodeIndex, include_type_edges: bool) -> Vec<NodeIndex> {
        self.neighbors(node, Direction::Incoming, include_type_edges)
    }

    fn neighbors(
        &self,
        node: NodeIndex,
        direction: Direction,
        include_type_edges: bool,
    ) -> Vec<NodeIndex> {
        let mut out: Vec<NodeIndex> = self
            .graph
            .edges_directed(node, direction)
            .filter(|e| include_type_edges || e.weight().is_runtime())
            .map(|e| match direction {
                Direction::Outgoing => petgraph::visit::EdgeRef::target(&e),
                Direction::Incoming => petgraph::visit::EdgeRef::source(&e),
            })
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Strongly-connected components, in reverse topological order.
    ///
    /// Computed over the *filtered* graph. Running `tarjan_scc` on the unfiltered
    /// graph and filtering only during traversal would report components joined
    /// solely by type-only edges — which do not exist at runtime — and would inflate
    /// both cycle reports and reachability counts.
    pub fn sccs(&self, include_type_edges: bool) -> Vec<Vec<NodeIndex>> {
        if include_type_edges {
            petgraph::algo::tarjan_scc(&self.graph)
        } else {
            let filtered = petgraph::visit::EdgeFiltered::from_fn(&self.graph, |edge| {
                petgraph::visit::EdgeRef::weight(&edge).is_runtime()
            });
            petgraph::algo::tarjan_scc(&filtered)
        }
    }

    pub fn edge_kind_counts(&self) -> Vec<(EdgeKind, usize)> {
        let mut counts: HashMap<EdgeKind, usize> = HashMap::new();
        for edge in self.graph.edge_indices() {
            if let Some(kind) = self.graph.edge_weight(edge) {
                *counts.entry(*kind).or_default() += 1;
            }
        }
        let mut counts: Vec<_> = counts.into_iter().collect();
        counts.sort_by_key(|(kind, _)| *kind);
        counts
    }
}

/// Every (specifier, edge kind) this module contributes, in source order.
fn edge_kinds(facts: &ModuleFacts) -> Vec<(&str, EdgeKind)> {
    let mut edges = Vec::new();

    for import in &facts.imports {
        let kind = if import.is_type_only() {
            EdgeKind::TypeOnly
        } else {
            match import.kind {
                ImportKind::Dynamic => EdgeKind::Dynamic,
                ImportKind::SideEffect => EdgeKind::SideEffectOnly,
                _ => EdgeKind::Static,
            }
        };
        edges.push((import.specifier.as_str(), kind));
    }

    for export in &facts.exports {
        if let Some(from) = export.from_specifier() {
            let type_only = match export {
                crate::facts::ExportRecord::Named { type_only, .. } => *type_only,
                _ => false,
            };
            let kind = if type_only {
                EdgeKind::TypeOnly
            } else {
                EdgeKind::ReExport
            };
            edges.push((from, kind));
        }
    }

    edges
}
