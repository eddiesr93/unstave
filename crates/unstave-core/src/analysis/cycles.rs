use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};

use crate::graph::ModuleGraph;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cycle {
    /// Every module in the strongly-connected component.
    pub members: Vec<PathBuf>,
    /// One concrete shortest cycle inside the component, starting and ending at the
    /// same module. A whole SCC is hard to act on; a single closed path is not.
    pub shortest_path: Vec<PathBuf>,
}

impl Cycle {
    pub fn size(&self) -> usize {
        self.members.len()
    }
}

/// Strongly-connected components of size > 1, each with one concrete shortest cycle.
///
/// Self-loops (a module importing itself) are reported too — they are legal to write
/// and always a mistake.
pub fn find(graph: &ModuleGraph, include_type_edges: bool) -> Vec<Cycle> {
    let mut cycles: Vec<Cycle> = graph
        .sccs(include_type_edges)
        .into_iter()
        .filter_map(|component| build_cycle(graph, &component, include_type_edges))
        .collect();

    // Biggest components first — they are the ones worth untangling.
    cycles.sort_by(|a, b| {
        b.size()
            .cmp(&a.size())
            .then_with(|| a.members.cmp(&b.members))
    });
    cycles
}

fn build_cycle(
    graph: &ModuleGraph,
    component: &[NodeIndex],
    include_type_edges: bool,
) -> Option<Cycle> {
    if component.len() == 1 {
        let node = component[0];
        // tarjan_scc reports a lone node as its own component; it is only a cycle if
        // it actually links to itself.
        if !graph.successors(node, include_type_edges).contains(&node) {
            return None;
        }
        let path = graph.path_of(node).to_path_buf();
        return Some(Cycle {
            members: vec![path.clone()],
            shortest_path: vec![path.clone(), path],
        });
    }

    let in_component: HashSet<NodeIndex> = component.iter().copied().collect();
    let start = *component
        .iter()
        .min_by_key(|n| graph.path_of(**n).to_path_buf())?;

    let path = shortest_cycle_through(graph, start, &in_component, include_type_edges)?;

    let mut members: Vec<PathBuf> = component
        .iter()
        .map(|n| graph.path_of(*n).to_path_buf())
        .collect();
    members.sort();

    Some(Cycle {
        members,
        shortest_path: path
            .into_iter()
            .map(|n| graph.path_of(n).to_path_buf())
            .collect(),
    })
}

/// BFS from `start` back to `start`, staying inside the component.
fn shortest_cycle_through(
    graph: &ModuleGraph,
    start: NodeIndex,
    component: &HashSet<NodeIndex>,
    include_type_edges: bool,
) -> Option<Vec<NodeIndex>> {
    let mut previous: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();

    for next in graph.successors(start, include_type_edges) {
        if component.contains(&next) && visited.insert(next) {
            previous.insert(next, start);
            queue.push_back(next);
        }
    }

    while let Some(node) = queue.pop_front() {
        if node == start {
            break;
        }
        for next in graph.successors(node, include_type_edges) {
            if !component.contains(&next) {
                continue;
            }
            if next == start {
                // Walk `previous` back from `node` to `start`, collecting the interior
                // of the cycle, then bracket it with `start` at both ends.
                let mut interior = vec![node];
                let mut cursor = node;
                while let Some(&prev) = previous.get(&cursor) {
                    if prev == start {
                        break;
                    }
                    interior.push(prev);
                    cursor = prev;
                }
                interior.reverse();

                let mut path = Vec::with_capacity(interior.len() + 2);
                path.push(start);
                path.extend(interior);
                path.push(start);
                return Some(path);
            }
            if visited.insert(next) {
                previous.insert(next, node);
                queue.push_back(next);
            }
        }
    }

    None
}
