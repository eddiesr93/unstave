use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;

use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};

use crate::analysis::symbols::{Resolution, SymbolResolver};
use crate::facts::{ExportRecord, ImportKind};
use crate::graph::ModuleGraph;
use crate::pipeline::{Analysis, Module};
use crate::resolve::Resolved;

/// An exported definition with no statically visible inbound reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadExport {
    pub module: PathBuf,
    pub name: String,
    /// `export *`, namespace imports, or dynamic imports touch the definition's
    /// module, so property access that the AST cannot see may still use it.
    pub low_confidence: bool,
}

/// Find exported definitions that no import in the workspace references.
///
/// Configured entrypoint modules are excluded. Package `exports` targets and their
/// complete dependency closures are treated as public API and excluded too.
pub fn find(
    analysis: &Analysis,
    graph: &ModuleGraph,
    symbols: &SymbolResolver<'_>,
    entrypoints: &[PathBuf],
) -> Vec<DeadExport> {
    let candidates = exported_definitions(graph, symbols);
    let (used, uncertain_modules) = references(&analysis.modules, graph, symbols);
    let excluded = excluded_modules(analysis, graph, entrypoints);

    candidates
        .into_iter()
        .filter(|(key, _aliases)| !used.contains(key) && !excluded.contains(&key.0))
        .map(|((module, name), _aliases)| DeadExport {
            low_confidence: uncertain_modules.contains(&module),
            module,
            name,
        })
        .collect()
}

/// Definition key → the public aliases that resolve to it. The aliases are retained
/// internally so deduplication stays explicit even though the report names the real
/// declaration, which is the actionable edit site.
fn exported_definitions(
    graph: &ModuleGraph,
    symbols: &SymbolResolver<'_>,
) -> BTreeMap<(PathBuf, String), BTreeSet<String>> {
    let mut definitions = BTreeMap::new();
    for node in graph.node_indices() {
        for exported_name in graph
            .node(node)
            .facts
            .exports
            .iter()
            .filter_map(ExportRecord::exported_name)
        {
            if let Resolution::Definition { module, name } = symbols.resolve(node, exported_name) {
                definitions
                    .entry((module, name))
                    .or_insert_with(BTreeSet::new)
                    .insert(exported_name.to_string());
            }
        }
    }
    definitions
}

fn references(
    modules: &[Module],
    graph: &ModuleGraph,
    symbols: &SymbolResolver<'_>,
) -> (HashSet<(PathBuf, String)>, HashSet<PathBuf>) {
    let mut used = HashSet::new();
    let mut uncertain_roots = Vec::new();

    for module in modules {
        for import in &module.facts.imports {
            let Some(Resolved::Internal { path }) = module.resolutions.get(&import.specifier)
            else {
                continue;
            };
            let Some(target) = graph.index_of(path) else {
                continue;
            };

            if matches!(import.kind, ImportKind::Namespace | ImportKind::Dynamic) {
                uncertain_roots.push(target);
                continue;
            }
            for binding in &import.bindings {
                if binding.imported == "*" {
                    uncertain_roots.push(target);
                    continue;
                }
                if let Resolution::Definition { module, name } =
                    symbols.resolve(target, &binding.imported)
                {
                    used.insert((module, name));
                }
            }
        }

        for export in &module.facts.exports {
            let ExportRecord::Star { from } = export else {
                continue;
            };
            if let Some(Resolved::Internal { path }) = module.resolutions.get(from) {
                if let Some(target) = graph.index_of(path) {
                    uncertain_roots.push(target);
                }
            }
        }
    }

    let uncertain_modules = closure_paths(graph, uncertain_roots, true);
    (used, uncertain_modules)
}

fn excluded_modules(
    analysis: &Analysis,
    graph: &ModuleGraph,
    entrypoints: &[PathBuf],
) -> HashSet<PathBuf> {
    let configured = entrypoints
        .iter()
        .filter_map(|path| graph.index_of(path))
        .collect::<Vec<_>>();
    let mut excluded = configured
        .into_iter()
        .map(|node| graph.path_of(node).to_path_buf())
        .collect::<HashSet<_>>();

    let public = analysis
        .workspace
        .packages
        .iter()
        .flat_map(|package| &package.public_entrypoints)
        .filter_map(|path| graph.index_of(path))
        .collect::<Vec<_>>();
    excluded.extend(closure_paths(graph, public, true));
    excluded
}

fn closure_paths(
    graph: &ModuleGraph,
    roots: Vec<NodeIndex>,
    include_type_edges: bool,
) -> HashSet<PathBuf> {
    graph
        .closure_from(roots, |node| graph.successors(node, include_type_edges))
        .into_iter()
        .map(|node| graph.path_of(node).to_path_buf())
        .collect()
}
