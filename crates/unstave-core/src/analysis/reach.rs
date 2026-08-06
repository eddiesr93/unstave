use std::collections::HashMap;

use fixedbitset::FixedBitSet;
use petgraph::graph::NodeIndex;

use crate::graph::ModuleGraph;

/// Transitive reachability over the module graph.
///
/// Computed once by condensing strongly-connected components and unioning reachable
/// sets in reverse topological order, so a module inside a cycle costs no more than a
/// module outside one. Naive per-node BFS would repeat that work for every node and
/// is what makes this analysis slow on large repos.
pub struct Reachability {
    /// Reachable set per node, as a bitset over dense positions.
    sets: Vec<FixedBitSet>,
    /// Node index → position in `sets`.
    slot: HashMap<NodeIndex, usize>,
    /// Position → node index, the inverse of `slot`.
    nodes: Vec<NodeIndex>,
}

impl Reachability {
    /// Build reachability following outgoing edges (what each module pulls in).
    pub fn forward(graph: &ModuleGraph, include_type_edges: bool) -> Self {
        Self::build(graph, include_type_edges, Direction::Forward)
    }

    /// Build reachability following incoming edges (what pulls each module in).
    pub fn reverse(graph: &ModuleGraph, include_type_edges: bool) -> Self {
        Self::build(graph, include_type_edges, Direction::Reverse)
    }

    fn build(graph: &ModuleGraph, include_type_edges: bool, direction: Direction) -> Self {
        let nodes: Vec<NodeIndex> = graph.node_indices().collect();
        let node_count = nodes.len();

        // Dense positions so bitsets can be indexed without a hash lookup per bit.
        let mut position = HashMap::with_capacity(node_count);
        for (i, n) in nodes.iter().enumerate() {
            position.insert(*n, i);
        }

        let neighbors = |n: NodeIndex| -> Vec<NodeIndex> {
            match direction {
                Direction::Forward => graph.successors(n, include_type_edges),
                Direction::Reverse => graph.predecessors(n, include_type_edges),
            }
        };

        // `sccs` returns components in reverse topological order with respect to
        // *forward* edges: for an edge A -> B, B's component comes first. That is the
        // order we want going forward, since a component's successors are then already
        // finished. Traversing predecessors needs the opposite order — B is processed
        // before A there, so A must come first.
        let mut components = graph.sccs(include_type_edges);
        if matches!(direction, Direction::Reverse) {
            components.reverse();
        }

        let mut sets: Vec<FixedBitSet> = vec![FixedBitSet::with_capacity(node_count); node_count];
        let mut done: Vec<bool> = vec![false; node_count];

        for component in &components {
            // Everything reachable from anywhere in the component, including the
            // component itself when it is a real cycle.
            let mut shared = FixedBitSet::with_capacity(node_count);

            for &node in component {
                for next in neighbors(node) {
                    let Some(&next_pos) = position.get(&next) else {
                        continue;
                    };
                    shared.insert(next_pos);
                    if done[next_pos] {
                        shared.union_with(&sets[next_pos]);
                    }
                }
            }

            // Members of a multi-node SCC all reach each other, so they share one set.
            for &node in component {
                let Some(&pos) = position.get(&node) else {
                    continue;
                };
                sets[pos] = shared.clone();
                done[pos] = true;
            }
        }

        Self {
            sets,
            slot: position,
            nodes,
        }
    }

    /// Number of distinct modules reachable from `node`, excluding itself unless it
    /// participates in a cycle.
    pub fn count(&self, node: NodeIndex) -> usize {
        self.slot
            .get(&node)
            .map(|&pos| self.sets[pos].count_ones(..))
            .unwrap_or(0)
    }

    /// Whether `target` is reachable from `node` under this instance's edge filter.
    pub fn reaches(&self, node: NodeIndex, target: NodeIndex) -> bool {
        let (Some(&from), Some(&to)) = (self.slot.get(&node), self.slot.get(&target)) else {
            return false;
        };
        self.sets[from].contains(to)
    }

    /// The reachable set from `node`, as node indices.
    pub fn set(&self, node: NodeIndex) -> Vec<NodeIndex> {
        let Some(&pos) = self.slot.get(&node) else {
            return Vec::new();
        };
        self.sets[pos].ones().map(|bit| self.nodes[bit]).collect()
    }

    /// Union of the reachable sets of `nodes`, plus the nodes themselves.
    ///
    /// This is the "minimal cost" primitive for barrel amplification: the true cost of
    /// importing a set of symbols is the union of their closures, not the sum.
    pub fn union_count(&self, nodes: &[NodeIndex]) -> usize {
        let mut union = FixedBitSet::with_capacity(self.nodes.len());
        for node in nodes {
            let Some(&pos) = self.slot.get(node) else {
                continue;
            };
            union.insert(pos);
            union.union_with(&self.sets[pos]);
        }
        union.count_ones(..)
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Forward,
    Reverse,
}
