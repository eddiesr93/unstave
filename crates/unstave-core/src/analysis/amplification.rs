use std::collections::HashMap;
use std::path::PathBuf;

use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};

use crate::analysis::barrel::Barrel;
use crate::analysis::reach::Reachability;
use crate::analysis::symbols::{Resolution, SymbolResolver};
use crate::facts::Span;
use crate::graph::ModuleGraph;
use crate::pipeline::Module;

/// Re-exported so `unstave_core::analysis::amplification::SkipReason` keeps
/// resolving to the single shared [`SkipReason`].
pub use crate::analysis::skip::SkipReason;

/// One import statement that goes through a barrel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSite {
    pub importer: PathBuf,
    pub barrel: PathBuf,
    pub span: Span,
    /// Names actually imported here.
    pub symbols: Vec<String>,
    /// Modules reachable from the barrel — what this import really costs today.
    pub actual_cost: usize,
    /// Modules reachable from just the definition sites of `symbols`.
    pub minimal_cost: usize,
    /// Symbols that could be rewritten, with their definition sites.
    pub rewritable: Vec<(String, PathBuf)>,
    /// Symbols that could not, and why.
    pub skipped: Vec<(String, SkipReason)>,
}

impl ImportSite {
    /// Modules that would leave the graph if this one import were rewritten.
    pub fn excess(&self) -> usize {
        self.actual_cost.saturating_sub(self.minimal_cost)
    }

    /// How many times more than necessary this import costs.
    pub fn amplification(&self) -> f64 {
        if self.minimal_cost == 0 {
            // Nothing resolved, so there is no meaningful ratio to report.
            return if self.actual_cost == 0 {
                1.0
            } else {
                f64::INFINITY
            };
        }
        self.actual_cost as f64 / self.minimal_cost as f64
    }

    pub fn is_fully_rewritable(&self) -> bool {
        self.skipped.is_empty() && !self.rewritable.is_empty()
    }
}

/// Aggregate for one barrel across all the places that import it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BarrelAmplification {
    pub barrel: PathBuf,
    pub import_sites: usize,
    /// Transitive closure of the barrel itself.
    pub actual_cost: usize,
    /// Worst single import site, by excess.
    pub worst_excess: usize,
    /// Sum of excess across every import site. Not a module count — the same module
    /// can be counted once per importer — but it is the right ranking signal, since
    /// each importer pays its own cost in the dev server graph.
    pub total_excess: usize,
    pub max_amplification: f64,
    pub has_side_effects: bool,
    pub rewritable_symbols: usize,
    pub skipped_symbols: usize,
}

/// Projected effect of unwinding every eligible barrel import, per entrypoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntrypointProjection {
    pub entrypoint: PathBuf,
    /// Transitive module count today.
    pub before: usize,
    /// Transitive module count after a hypothetical full codemod.
    pub after: usize,
}

impl EntrypointProjection {
    pub fn removed(&self) -> usize {
        self.before.saturating_sub(self.after)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmplificationReport {
    /// Modules that met the barrel thresholds, whether or not anything imports them.
    pub classified_barrels: usize,
    pub sites: Vec<ImportSite>,
    pub barrels: Vec<BarrelAmplification>,
    pub entrypoints: Vec<EntrypointProjection>,
    pub skipped_by_reason: Vec<(SkipReason, usize)>,
}

/// Compute barrel amplification across the workspace.
pub fn compute(
    graph: &ModuleGraph,
    modules: &[Module],
    barrels: &[Barrel],
    symbols: &SymbolResolver<'_>,
    entrypoints: &[PathBuf],
    include_type_edges: bool,
) -> AmplificationReport {
    let forward = Reachability::forward(graph, include_type_edges);
    let barrel_nodes: HashMap<NodeIndex, &Barrel> = barrels.iter().map(|b| (b.node, b)).collect();

    let mut sites = Vec::new();

    for module in modules {
        let Some(importer_node) = graph.index_of(&module.facts.path) else {
            continue;
        };

        for import in &module.facts.imports {
            // Type-only imports cost nothing at runtime.
            if import.is_type_only() && !include_type_edges {
                continue;
            }
            let Some(crate::resolve::Resolved::Internal { path }) =
                module.resolutions.get(&import.specifier)
            else {
                continue;
            };
            let Some(target) = graph.index_of(path) else {
                continue;
            };
            let Some(barrel) = barrel_nodes.get(&target) else {
                continue;
            };
            // A module importing its own barrel is not an amplification site.
            if target == importer_node {
                continue;
            }

            sites.push(build_site(
                graph, symbols, &forward, barrel, module, import, target,
            ));
        }
    }

    sites.sort_by(|a, b| {
        b.excess()
            .cmp(&a.excess())
            .then_with(|| a.importer.cmp(&b.importer))
            .then_with(|| a.span.start.cmp(&b.span.start))
    });

    let barrels_report = aggregate(&sites, barrels);
    let entrypoints = project_entrypoints(graph, &forward, &sites, entrypoints, include_type_edges);
    let skipped_by_reason = count_skips(&sites);

    AmplificationReport {
        classified_barrels: barrels.len(),
        sites,
        barrels: barrels_report,
        entrypoints,
        skipped_by_reason,
    }
}

fn build_site(
    graph: &ModuleGraph,
    symbols: &SymbolResolver<'_>,
    forward: &Reachability,
    barrel: &Barrel,
    module: &Module,
    import: &crate::facts::ImportRecord,
    target: NodeIndex,
) -> ImportSite {
    // The barrel itself plus everything behind it.
    let actual_cost = forward.count(target) + 1;

    let mut rewritable = Vec::new();
    let mut skipped = Vec::new();
    let mut definition_nodes: Vec<NodeIndex> = Vec::new();
    let mut symbol_names = Vec::new();

    for binding in &import.bindings {
        symbol_names.push(binding.imported.clone());

        if binding.imported == "*" {
            skipped.push((binding.local.clone(), SkipReason::NamespaceImport));
            // A namespace import genuinely needs the whole barrel.
            definition_nodes.push(target);
            continue;
        }

        let resolution = symbols.resolve(target, &binding.imported);
        match &resolution {
            Resolution::Definition { module: def, .. } => {
                if let Some(def_node) = graph.index_of(def) {
                    definition_nodes.push(def_node);
                }
                if barrel.has_side_effects {
                    skipped.push((binding.imported.clone(), SkipReason::BarrelHasSideEffects));
                } else {
                    rewritable.push((binding.imported.clone(), def.clone()));
                }
            }
            Resolution::Ambiguous { .. } => {
                skipped.push((binding.imported.clone(), SkipReason::Ambiguous));
                definition_nodes.push(target);
            }
            Resolution::Cyclic => {
                skipped.push((binding.imported.clone(), SkipReason::Cyclic));
                definition_nodes.push(target);
            }
            Resolution::External { .. } => {
                skipped.push((binding.imported.clone(), SkipReason::External));
            }
            Resolution::NotFound => {
                skipped.push((binding.imported.clone(), SkipReason::NotFound));
                definition_nodes.push(target);
            }
        }
    }

    // The union, not the sum: two symbols sharing a dependency pay for it once.
    let minimal_cost = forward.union_count(&definition_nodes);

    ImportSite {
        importer: module.facts.path.clone(),
        barrel: barrel.path.clone(),
        span: import.span,
        symbols: symbol_names,
        actual_cost,
        minimal_cost,
        rewritable,
        skipped,
    }
}

fn aggregate(sites: &[ImportSite], barrels: &[Barrel]) -> Vec<BarrelAmplification> {
    let mut by_barrel: HashMap<&PathBuf, Vec<&ImportSite>> = HashMap::new();
    for site in sites {
        by_barrel.entry(&site.barrel).or_default().push(site);
    }

    let mut report: Vec<BarrelAmplification> = by_barrel
        .into_iter()
        .map(|(path, sites)| {
            let has_side_effects = barrels
                .iter()
                .find(|b| &b.path == path)
                .is_some_and(|b| b.has_side_effects);

            BarrelAmplification {
                barrel: path.clone(),
                import_sites: sites.len(),
                actual_cost: sites.first().map(|s| s.actual_cost).unwrap_or(0),
                worst_excess: sites.iter().map(|s| s.excess()).max().unwrap_or(0),
                total_excess: sites.iter().map(|s| s.excess()).sum(),
                max_amplification: sites
                    .iter()
                    .map(|s| s.amplification())
                    .fold(0.0_f64, f64::max),
                has_side_effects,
                rewritable_symbols: sites.iter().map(|s| s.rewritable.len()).sum(),
                skipped_symbols: sites.iter().map(|s| s.skipped.len()).sum(),
            }
        })
        .collect();

    // §5.3: rank by absolute module count, not ratio — that is what costs dev time.
    report.sort_by(|a, b| {
        b.total_excess
            .cmp(&a.total_excess)
            .then_with(|| a.barrel.cmp(&b.barrel))
    });
    report
}

/// Per-entrypoint transitive module count, before and after a hypothetical codemod.
///
/// "After" is computed by rebuilding reachability on a graph where every fully
/// rewritable barrel import is replaced by edges to the definition sites. Partially
/// rewritable sites keep their barrel edge, so the projection is a lower bound on the
/// saving rather than an optimistic one.
fn project_entrypoints(
    graph: &ModuleGraph,
    forward: &Reachability,
    sites: &[ImportSite],
    entrypoints: &[PathBuf],
    include_type_edges: bool,
) -> Vec<EntrypointProjection> {
    if entrypoints.is_empty() {
        return Vec::new();
    }

    // importer → (barrel → definition sites) for sites we could actually rewrite.
    let mut rewrites: HashMap<PathBuf, HashMap<PathBuf, Vec<PathBuf>>> = HashMap::new();
    for site in sites.iter().filter(|s| s.is_fully_rewritable()) {
        rewrites
            .entry(site.importer.clone())
            .or_default()
            .entry(site.barrel.clone())
            .or_default()
            .extend(site.rewritable.iter().map(|(_, path)| path.clone()));
    }

    entrypoints
        .iter()
        .filter_map(|entry| {
            let node = graph.index_of(entry)?;
            let before = forward.count(node) + 1;
            let after = projected_closure(graph, forward, &rewrites, node, include_type_edges);
            Some(EntrypointProjection {
                entrypoint: entry.clone(),
                before,
                after,
            })
        })
        .collect()
}

/// BFS from `entry` over the graph as it would be after rewriting.
fn projected_closure(
    graph: &ModuleGraph,
    forward: &Reachability,
    rewrites: &HashMap<PathBuf, HashMap<PathBuf, Vec<PathBuf>>>,
    entry: NodeIndex,
    include_type_edges: bool,
) -> usize {
    let mut seen = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    seen.insert(entry);
    queue.push_back(entry);

    while let Some(node) = queue.pop_front() {
        let path = graph.path_of(node).to_path_buf();
        let redirects = rewrites.get(&path);

        for next in graph.successors(node, include_type_edges) {
            let next_path = graph.path_of(next).to_path_buf();

            // If this importer's edge to that barrel is fully rewritable, the barrel
            // and its closure are replaced by the definition sites.
            if let Some(targets) = redirects.and_then(|r| r.get(&next_path)) {
                for target in targets {
                    let Some(target_node) = graph.index_of(target) else {
                        continue;
                    };
                    // The definition may sit behind a re-export this edge filter drops
                    // — a `export type { X }` chain when type edges are excluded. Such
                    // a module was never in the "before" closure, so counting it here
                    // would make the projection grow instead of shrink.
                    if !forward.reaches(next, target_node) {
                        continue;
                    }
                    if seen.insert(target_node) {
                        queue.push_back(target_node);
                    }
                }
                continue;
            }

            if seen.insert(next) {
                queue.push_back(next);
            }
        }
    }

    seen.len()
}

fn count_skips(sites: &[ImportSite]) -> Vec<(SkipReason, usize)> {
    let mut counts: HashMap<SkipReason, usize> = HashMap::new();
    for site in sites {
        for (_, reason) in &site.skipped {
            *counts.entry(*reason).or_default() += 1;
        }
    }
    let mut counts: Vec<_> = counts.into_iter().collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    counts
}
