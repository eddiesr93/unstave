//! Stable, renderer-neutral analysis report.
//!
//! Paths are workspace-relative and use forward slashes so JSON snapshots and CI
//! artifacts do not depend on the machine that produced them.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use serde::Serialize;
use unstave_core::analysis::amplification::{self, SkipReason};
use unstave_core::analysis::barrel::{self, Barrel, BarrelKind};
use unstave_core::analysis::symbols::SymbolResolver;
use unstave_core::analysis::{cycles, dead_exports, fan};
use unstave_core::discovery::WorkspaceKind;
use unstave_core::facts::Span;
use unstave_core::graph::{EdgeKind, ModuleGraph};
use unstave_core::{Analysis, Config};

/// Current public JSON schema version.
pub const SCHEMA_VERSION: u32 = 2;

/// Everything every renderer needs, with no terminal or filesystem policy.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisReport {
    pub schema_version: u32,
    pub workspace: WorkspaceReport,
    pub summary: Summary,
    pub timings: TimingsReport,
    pub include_type_edges: bool,
    pub external_packages: Vec<String>,
    pub modules: Vec<ModuleReport>,
    pub edges: Vec<EdgeReport>,
    pub dangling_edges: Vec<DanglingEdgeReport>,
    pub unresolved_specifiers: Vec<UnresolvedReport>,
    pub parse_failures: Vec<ParseFailureReport>,
    pub cycles: Vec<CycleReport>,
    pub dead_exports: Vec<DeadExportReport>,
    pub fan: FanReport,
    pub amplification: AmplificationReport,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceReport {
    pub root: String,
    pub kind: WorkspaceKindReport,
    pub packages: Vec<PackageReport>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceKindReport {
    Pnpm,
    NpmWorkspaces,
    Single,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageReport {
    pub root: String,
    pub name: Option<String>,
    pub tsconfig: Option<String>,
    pub side_effects_free: bool,
    pub public_entrypoints: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub files_analyzed: usize,
    pub packages: usize,
    pub modules: usize,
    pub edges: usize,
    pub external_packages: usize,
    pub unresolved_specifiers: usize,
    pub parse_failures: usize,
    pub cycles: usize,
    pub dead_exports: usize,
    pub classified_barrels: usize,
    pub imported_barrels: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimingsReport {
    pub discovery_ms: u128,
    pub parse_ms: u128,
    pub resolve_ms: u128,
    pub total_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleReport {
    pub id: String,
    pub path: String,
    pub directory: String,
    pub has_side_effects: bool,
    pub barrel: Option<BarrelReport>,
    pub in_cycle: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarrelReport {
    pub kind: BarrelKind,
    pub export_count: usize,
    pub reexport_count: usize,
    pub own_decl_count: usize,
    pub reexport_ratio: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeReport {
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DanglingEdgeReport {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedReport {
    pub specifier: String,
    pub importer: String,
    pub span: Span,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseFailureReport {
    pub path: String,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CycleReport {
    pub members: Vec<String>,
    pub shortest_path: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadExportReport {
    pub module: String,
    pub name: String,
    pub low_confidence: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FanReport {
    pub fan_in: Vec<FanEntryReport>,
    pub fan_out: Vec<FanEntryReport>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FanEntryReport {
    pub path: String,
    pub direct: usize,
    pub transitive: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AmplificationReport {
    pub classified_barrels: usize,
    pub sites: Vec<ImportSiteReport>,
    pub barrels: Vec<BarrelAmplificationReport>,
    pub entrypoints: Vec<EntrypointProjectionReport>,
    pub skipped_by_reason: Vec<SkipCountReport>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSiteReport {
    pub importer: String,
    pub barrel: String,
    pub span: Span,
    pub symbols: Vec<String>,
    pub actual_cost: usize,
    pub minimal_cost: usize,
    pub excess: usize,
    /// Always a finite number: never `null`, never infinite.
    pub amplification: f64,
    pub rewritable: Vec<RewritableSymbolReport>,
    pub skipped: Vec<SkippedSymbolReport>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RewritableSymbolReport {
    pub symbol: String,
    pub definition: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedSymbolReport {
    pub symbol: String,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarrelAmplificationReport {
    pub barrel: String,
    pub import_sites: usize,
    pub actual_cost: usize,
    pub worst_excess: usize,
    pub total_excess: usize,
    /// Always a finite number: never `null`, never infinite.
    pub max_amplification: f64,
    pub has_side_effects: bool,
    pub rewritable_symbols: usize,
    pub skipped_symbols: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntrypointProjectionReport {
    pub entrypoint: String,
    pub before: usize,
    pub after: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkipCountReport {
    pub reason: SkipReason,
    pub symbols: usize,
}

/// Run the graph-level analyses once and produce the common renderer input.
pub fn build(
    analysis: &Analysis,
    graph: &ModuleGraph,
    config: &Config,
    include_type_edges: bool,
) -> AnalysisReport {
    let root = &analysis.workspace.root;
    let symbols = SymbolResolver::new(graph, &analysis.modules);
    let classified_barrels = barrel::classify(graph, &config.barrel);
    let entrypoints = config.entrypoint_paths(root);
    let amplification = amplification::compute(
        graph,
        &analysis.modules,
        &classified_barrels,
        &symbols,
        &entrypoints,
        include_type_edges,
    );
    let found_cycles = cycles::find(graph, include_type_edges);
    let found_dead_exports = dead_exports::find(analysis, graph, &symbols, &entrypoints);
    // JSON and HTML are complete. Terminal truncation happens while rendering.
    let fan = fan::compute(graph, include_type_edges, usize::MAX);

    let cycle_members: HashSet<_> = found_cycles
        .iter()
        .flat_map(|cycle| cycle.members.iter().cloned())
        .collect();
    let barrels_by_path: HashMap<_, _> = classified_barrels
        .iter()
        .map(|barrel| (barrel.path.as_path(), barrel))
        .collect();

    let mut nodes: Vec<_> = graph.node_indices().collect();
    nodes.sort_by(|a, b| graph.path_of(*a).cmp(graph.path_of(*b)));
    let mut node_ids = HashMap::new();
    let modules: Vec<ModuleReport> = nodes
        .into_iter()
        .enumerate()
        .map(|(position, node)| {
            let path = graph.path_of(node);
            let id = format!("m{position}");
            node_ids.insert(node, id.clone());
            let display = path_string(root, path);
            ModuleReport {
                id,
                directory: directory_of(&display),
                path: display,
                has_side_effects: graph.node(node).facts.has_side_effects,
                barrel: barrels_by_path
                    .get(path)
                    .map(|barrel| barrel_report(barrel)),
                in_cycle: cycle_members.contains(path),
            }
        })
        .collect();

    let mut edges: Vec<EdgeReport> = graph
        .inner()
        .edge_references()
        .map(|edge| EdgeReport {
            source: node_ids[&edge.source()].clone(),
            target: node_ids[&edge.target()].clone(),
            kind: *edge.weight(),
        })
        .collect();
    edges.sort_by(|a, b| {
        a.source
            .cmp(&b.source)
            .then_with(|| a.target.cmp(&b.target))
            .then_with(|| a.kind.cmp(&b.kind))
    });

    let external_packages = analysis
        .external_packages()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let unresolved_specifiers = analysis
        .unresolved
        .iter()
        .map(|item| UnresolvedReport {
            specifier: item.specifier.clone(),
            importer: path_string(root, &item.importer),
            span: item.span,
            reason: normalize_text(root, &item.reason),
        })
        .collect::<Vec<_>>();
    let parse_failures = analysis
        .modules
        .iter()
        .filter(|module| !module.facts.parse_errors.is_empty())
        .map(|module| ParseFailureReport {
            path: path_string(root, &module.facts.path),
            errors: module
                .facts
                .parse_errors
                .iter()
                .map(|error| normalize_text(root, error))
                .collect(),
        })
        .collect::<Vec<_>>();

    let report_amplification = amplification_report(root, &amplification);
    let summary = Summary {
        files_analyzed: analysis.modules.len(),
        packages: analysis.workspace.packages.len(),
        modules: graph.node_count(),
        edges: graph.edge_count(),
        external_packages: external_packages.len(),
        unresolved_specifiers: unresolved_specifiers.len(),
        parse_failures: parse_failures.len(),
        cycles: found_cycles.len(),
        dead_exports: found_dead_exports.len(),
        classified_barrels: amplification.classified_barrels,
        imported_barrels: amplification.barrels.len(),
    };

    AnalysisReport {
        schema_version: SCHEMA_VERSION,
        workspace: workspace_report(analysis),
        summary,
        timings: TimingsReport {
            discovery_ms: analysis.timings.discovery_ms,
            parse_ms: analysis.timings.parse_ms,
            resolve_ms: analysis.timings.resolve_ms,
            total_ms: analysis.timings.total_ms,
        },
        include_type_edges,
        external_packages,
        modules,
        edges,
        dangling_edges: graph
            .dangling()
            .iter()
            .map(|(source, target)| DanglingEdgeReport {
                source: path_string(root, source),
                target: path_string(root, target),
            })
            .collect(),
        unresolved_specifiers,
        parse_failures,
        cycles: found_cycles
            .iter()
            .map(|cycle| CycleReport {
                members: cycle
                    .members
                    .iter()
                    .map(|path| path_string(root, path))
                    .collect(),
                shortest_path: cycle
                    .shortest_path
                    .iter()
                    .map(|path| path_string(root, path))
                    .collect(),
            })
            .collect(),
        dead_exports: found_dead_exports
            .iter()
            .map(|export| DeadExportReport {
                module: path_string(root, &export.module),
                name: export.name.clone(),
                low_confidence: export.low_confidence,
            })
            .collect(),
        fan: FanReport {
            fan_in: fan
                .fan_in
                .iter()
                .map(|entry| FanEntryReport {
                    path: path_string(root, &entry.path),
                    direct: entry.direct,
                    transitive: entry.transitive,
                })
                .collect(),
            fan_out: fan
                .fan_out
                .iter()
                .map(|entry| FanEntryReport {
                    path: path_string(root, &entry.path),
                    direct: entry.direct,
                    transitive: entry.transitive,
                })
                .collect(),
        },
        amplification: report_amplification,
    }
}

fn workspace_report(analysis: &Analysis) -> WorkspaceReport {
    let root = &analysis.workspace.root;
    let mut packages = analysis
        .workspace
        .packages
        .iter()
        .map(|package| PackageReport {
            root: path_string(root, &package.root),
            name: package.name.clone(),
            tsconfig: package
                .tsconfig
                .as_ref()
                .map(|path| path_string(root, path)),
            side_effects_free: package.side_effects_false,
            public_entrypoints: package
                .public_entrypoints
                .iter()
                .map(|path| path_string(root, path))
                .collect(),
        })
        .collect::<Vec<_>>();
    packages.sort_by(|a, b| a.root.cmp(&b.root));

    WorkspaceReport {
        // The report is deliberately relocatable; `.` means the analyzed root.
        root: ".".to_string(),
        kind: match analysis.workspace.kind {
            WorkspaceKind::Pnpm => WorkspaceKindReport::Pnpm,
            WorkspaceKind::NpmWorkspaces => WorkspaceKindReport::NpmWorkspaces,
            WorkspaceKind::Single => WorkspaceKindReport::Single,
        },
        packages,
    }
}

fn barrel_report(barrel: &Barrel) -> BarrelReport {
    BarrelReport {
        kind: barrel.kind,
        export_count: barrel.export_count,
        reexport_count: barrel.reexport_count,
        own_decl_count: barrel.own_decl_count,
        reexport_ratio: barrel.reexport_ratio(),
    }
}

fn amplification_report(
    root: &Path,
    source: &amplification::AmplificationReport,
) -> AmplificationReport {
    AmplificationReport {
        classified_barrels: source.classified_barrels,
        sites: source
            .sites
            .iter()
            .map(|site| ImportSiteReport {
                importer: path_string(root, &site.importer),
                barrel: path_string(root, &site.barrel),
                span: site.span,
                symbols: site.symbols.clone(),
                actual_cost: site.actual_cost,
                minimal_cost: site.minimal_cost,
                excess: site.excess(),
                amplification: site.amplification(),
                rewritable: site
                    .rewritable
                    .iter()
                    .map(|(symbol, definition)| RewritableSymbolReport {
                        symbol: symbol.clone(),
                        definition: path_string(root, definition),
                    })
                    .collect(),
                skipped: site
                    .skipped
                    .iter()
                    .map(|(symbol, reason)| SkippedSymbolReport {
                        symbol: symbol.clone(),
                        reason: *reason,
                    })
                    .collect(),
            })
            .collect(),
        barrels: source
            .barrels
            .iter()
            .map(|barrel| BarrelAmplificationReport {
                barrel: path_string(root, &barrel.barrel),
                import_sites: barrel.import_sites,
                actual_cost: barrel.actual_cost,
                worst_excess: barrel.worst_excess,
                total_excess: barrel.total_excess,
                max_amplification: barrel.max_amplification,
                has_side_effects: barrel.has_side_effects,
                rewritable_symbols: barrel.rewritable_symbols,
                skipped_symbols: barrel.skipped_symbols,
            })
            .collect(),
        entrypoints: source
            .entrypoints
            .iter()
            .map(|entrypoint| EntrypointProjectionReport {
                entrypoint: path_string(root, &entrypoint.entrypoint),
                before: entrypoint.before,
                after: entrypoint.after,
                removed: entrypoint.removed(),
            })
            .collect(),
        skipped_by_reason: source
            .skipped_by_reason
            .iter()
            .map(|(reason, symbols)| SkipCountReport {
                reason: *reason,
                symbols: *symbols,
            })
            .collect(),
    }
}

fn directory_of(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(directory, _)| directory.to_string())
        .unwrap_or_else(|| ".".to_string())
}

/// Stable, forward-slash path for all public report artifacts.
pub(crate) fn path_string(root: &Path, path: &Path) -> String {
    let display = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
    let normalized = display.replace('\\', "/");
    if normalized.is_empty() {
        ".".to_string()
    } else {
        normalized
    }
}

fn normalize_text(root: &Path, text: &str) -> String {
    let root = root.to_string_lossy().replace('\\', "/");
    text.replace('\\', "/").replace(&root, ".")
}
