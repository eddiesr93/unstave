use std::collections::{BTreeMap, BTreeSet, HashMap};

use unstave_core::graph::EdgeKind;

use crate::AnalysisReport;

#[derive(Debug)]
pub(crate) struct Projection {
    pub nodes: Vec<VisualNode>,
    pub edges: Vec<VisualEdge>,
    pub collapsed: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct VisualNode {
    pub id: String,
    pub label: String,
    pub directory: String,
    pub module_count: usize,
    pub is_barrel: bool,
    pub in_cycle: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct VisualEdge {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
}

pub(crate) fn project(report: &AnalysisReport, max_nodes: usize) -> Projection {
    let max_nodes = max_nodes.max(1);
    if report.modules.len() <= max_nodes {
        return Projection {
            nodes: report
                .modules
                .iter()
                .map(|module| VisualNode {
                    id: module.id.clone(),
                    label: file_name(&module.path),
                    directory: module.directory.clone(),
                    module_count: 1,
                    is_barrel: module.barrel.is_some(),
                    in_cycle: module.in_cycle,
                })
                .collect(),
            edges: report
                .edges
                .iter()
                .map(|edge| VisualEdge {
                    source: edge.source.clone(),
                    target: edge.target.clone(),
                    kind: edge.kind,
                })
                .collect(),
            collapsed: false,
        };
    }

    let depth = deepest_grouping_within(report, max_nodes);
    let mut groups: BTreeMap<String, Vec<&crate::report::ModuleReport>> = BTreeMap::new();
    for module in &report.modules {
        groups
            .entry(directory_prefix(&module.path, depth))
            .or_default()
            .push(module);
    }

    let mut module_to_group = HashMap::new();
    let nodes = groups
        .into_iter()
        .enumerate()
        .map(|(index, (directory, modules))| {
            let id = format!("d{index}");
            for module in &modules {
                module_to_group.insert(module.id.clone(), id.clone());
            }
            VisualNode {
                id,
                label: format!("{directory}/ ({} modules)", modules.len()),
                directory: parent_directory(&directory),
                module_count: modules.len(),
                is_barrel: modules.iter().any(|module| module.barrel.is_some()),
                in_cycle: modules.iter().any(|module| module.in_cycle),
            }
        })
        .collect();

    let mut edges = BTreeSet::new();
    for edge in &report.edges {
        let Some(source) = module_to_group.get(&edge.source) else {
            continue;
        };
        let Some(target) = module_to_group.get(&edge.target) else {
            continue;
        };
        if source != target {
            edges.insert(VisualEdge {
                source: source.clone(),
                target: target.clone(),
                kind: edge.kind,
            });
        }
    }

    Projection {
        nodes,
        edges: edges.into_iter().collect(),
        collapsed: true,
    }
}

fn deepest_grouping_within(report: &AnalysisReport, max_nodes: usize) -> usize {
    let max_depth = report
        .modules
        .iter()
        .map(|module| directory_components(&module.path).len())
        .max()
        .unwrap_or(0);
    let mut selected = 0;
    for depth in 1..=max_depth {
        let count = report
            .modules
            .iter()
            .map(|module| directory_prefix(&module.path, depth))
            .collect::<BTreeSet<_>>()
            .len();
        if count > max_nodes {
            break;
        }
        selected = depth;
    }
    selected
}

fn directory_components(path: &str) -> Vec<&str> {
    let mut parts: Vec<_> = path.split('/').collect();
    parts.pop();
    parts
}

fn directory_prefix(path: &str, depth: usize) -> String {
    if depth == 0 {
        return ".".to_string();
    }
    let parts = directory_components(path);
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts[..parts.len().min(depth)].join("/")
    }
}

fn parent_directory(directory: &str) -> String {
    directory
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_else(|| ".".to_string())
}

fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_use_directories_not_file_names() {
        assert_eq!(directory_prefix("src/a/b.ts", 1), "src");
        assert_eq!(directory_prefix("src/a/b.ts", 2), "src/a");
        assert_eq!(directory_prefix("root.ts", 4), ".");
    }
}
