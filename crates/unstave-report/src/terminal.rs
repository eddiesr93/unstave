use std::fmt::Write as _;

use comfy_table::{presets::UTF8_FULL, Cell, ContentArrangement, Table};
use owo_colors::OwoColorize;
use unstave_core::analysis::amplification::{
    AmplificationReport, EntrypointProjection, ImportSite, SkipReason,
};
use unstave_core::analysis::cycles::Cycle;
use unstave_core::analysis::fan::{FanEntry, FanReport};
use unstave_core::graph::ModuleGraph;
use unstave_core::pipeline::{relative, Analysis};

use crate::RenderOptions;

/// Rows shown per section before truncating.
const DEFAULT_MAX_ROWS: usize = 20;

/// Discovery + resolution summary: what we found, and what we could not resolve.
pub fn render(analysis: &Analysis, opts: &RenderOptions) -> String {
    let mut out = String::new();
    let max_rows = if opts.max_rows == 0 {
        DEFAULT_MAX_ROWS
    } else {
        opts.max_rows
    };

    out.push_str(&summary_table(analysis, opts));
    out.push('\n');

    if !analysis.unresolved.is_empty() {
        out.push_str(&unresolved_table(analysis, opts, max_rows));
        out.push('\n');
    }

    let failures = analysis.parse_failures();
    if !failures.is_empty() {
        out.push_str(&parse_failure_table(analysis, &failures, opts, max_rows));
        out.push('\n');
    }

    let t = analysis.timings;
    let timing = format!(
        "discovery {}ms · parse {}ms · resolve {}ms · total {}ms",
        t.discovery_ms, t.parse_ms, t.resolve_ms, t.total_ms
    );
    let _ = writeln!(out, "{}", opts.dim(&timing));

    out
}

/// Graph-level report: size, cycles, and the fan-in/fan-out leaderboards.
pub fn render_graph(
    analysis: &Analysis,
    graph: &ModuleGraph,
    cycles: &[Cycle],
    fan: &FanReport,
    opts: &RenderOptions,
) -> String {
    let mut out = String::new();
    let max_rows = if opts.max_rows == 0 {
        DEFAULT_MAX_ROWS
    } else {
        opts.max_rows
    };
    let root = &analysis.workspace.root;

    let mut table = new_table();
    table.set_header(vec![header("metric", opts), header("value", opts)]);
    table.add_row(vec![Cell::new("modules"), Cell::new(graph.node_count())]);
    table.add_row(vec![Cell::new("edges"), Cell::new(graph.edge_count())]);
    for (kind, count) in graph.edge_kind_counts() {
        table.add_row(vec![Cell::new(format!("  {kind:?}")), Cell::new(count)]);
    }
    table.add_row(vec![Cell::new("cycles"), Cell::new(cycles.len())]);
    let _ = writeln!(out, "{table}\n");

    if !graph.dangling().is_empty() {
        let _ = writeln!(
            out,
            "{}\n",
            opts.dim(&format!(
                "{} import(s) point at files excluded from the analysis",
                graph.dangling().len()
            ))
        );
    }

    if cycles.is_empty() {
        let _ = writeln!(out, "{}\n", opts.bold("No cycles."));
    } else {
        out.push_str(&cycles_section(root, cycles, opts, max_rows));
    }

    out.push_str(&fan_table(
        "Fan-in (most depended upon)",
        root,
        &fan.fan_in,
        opts,
    ));
    out.push_str(&fan_table(
        "Fan-out (pulls in the most)",
        root,
        &fan.fan_out,
        opts,
    ));

    out
}

/// Barrel amplification — the headline report.
pub fn render_barrels(
    analysis: &Analysis,
    report: &AmplificationReport,
    min_amplification: Option<f64>,
    opts: &RenderOptions,
) -> String {
    let root = &analysis.workspace.root;
    let max_rows = if opts.max_rows == 0 {
        DEFAULT_MAX_ROWS
    } else {
        opts.max_rows
    };

    let barrels: Vec<_> = report
        .barrels
        .iter()
        .filter(|b| min_amplification.is_none_or(|min| b.max_amplification >= min))
        .collect();

    // A barrel nobody imports costs nothing, but saying "no barrels" when the
    // workspace has several would be misleading — report both numbers.
    let mut out = format!(
        "{}\n\n",
        opts.dim(&format!(
            "{} barrel(s) classified, {} of them imported",
            report.classified_barrels,
            report.barrels.len()
        ))
    );

    if barrels.is_empty() {
        let _ = writeln!(
            out,
            "{}",
            opts.bold("No barrel imports to report — nothing is importing through a barrel.")
        );
        return out;
    }

    // Ranked by absolute excess, per §5.3 — the ratio is shown but does not sort.
    let mut table = new_table();
    table.set_header(vec![
        header("barrel", opts),
        header("sites", opts),
        header("cost", opts),
        header("excess", opts),
        header("worst", opts),
        header("amp", opts),
        header("rewritable", opts),
    ]);
    for barrel in barrels.iter().take(max_rows) {
        let flag = if barrel.has_side_effects {
            " ⚠ side effects"
        } else {
            ""
        };
        table.add_row(vec![
            Cell::new(format!(
                "{}{flag}",
                relative(root, &barrel.barrel).display()
            )),
            Cell::new(barrel.import_sites),
            Cell::new(barrel.actual_cost),
            Cell::new(barrel.total_excess),
            Cell::new(barrel.worst_excess),
            Cell::new(format_ratio(barrel.max_amplification)),
            Cell::new(format!(
                "{}/{}",
                barrel.rewritable_symbols,
                barrel.rewritable_symbols + barrel.skipped_symbols
            )),
        ]);
    }
    let _ = writeln!(out, "{}\n{table}\n", opts.bold("Barrel amplification"));

    if barrels.len() > max_rows {
        let more = barrels.len() - max_rows;
        let _ = writeln!(
            out,
            "{}\n",
            opts.dim(&format!(
                "+{more} more, use --format json for the full list"
            ))
        );
    }

    out.push_str(&worst_sites_table(root, &report.sites, opts, max_rows));
    out.push_str(&projection_table(root, &report.entrypoints, opts));
    out.push_str(&skips_table(&report.skipped_by_reason, opts));
    out
}

fn worst_sites_table(
    root: &std::path::Path,
    sites: &[ImportSite],
    opts: &RenderOptions,
    max_rows: usize,
) -> String {
    let sites: Vec<&ImportSite> = sites.iter().filter(|s| s.excess() > 0).collect();
    if sites.is_empty() {
        return String::new();
    }

    let mut table = new_table();
    table.set_header(vec![
        header("import site", opts),
        header("symbols", opts),
        header("actual", opts),
        header("minimal", opts),
        header("excess", opts),
    ]);
    for site in sites.iter().take(max_rows) {
        table.add_row(vec![
            Cell::new(format!(
                "{} → {}",
                relative(root, &site.importer).display(),
                relative(root, &site.barrel).display()
            )),
            Cell::new(site.symbols.join(", ")),
            Cell::new(site.actual_cost),
            Cell::new(site.minimal_cost),
            Cell::new(site.excess()),
        ]);
    }
    format!("{}\n{table}\n\n", opts.bold("Worst import sites"))
}

fn projection_table(
    root: &std::path::Path,
    projections: &[EntrypointProjection],
    opts: &RenderOptions,
) -> String {
    if projections.is_empty() {
        return format!(
            "{}\n\n",
            opts.dim("No entrypoints configured — set `entrypoints` in unstave.toml to see the projected per-entrypoint saving.")
        );
    }

    let mut table = new_table();
    table.set_header(vec![
        header("entrypoint", opts),
        header("before", opts),
        header("after", opts),
        header("removed", opts),
    ]);
    for p in projections {
        let percent = if p.before == 0 {
            0.0
        } else {
            (p.removed() as f64 / p.before as f64) * 100.0
        };
        table.add_row(vec![
            Cell::new(relative(root, &p.entrypoint).display().to_string()),
            Cell::new(p.before),
            Cell::new(p.after),
            Cell::new(format!("{} ({percent:.0}%)", p.removed())),
        ]);
    }
    format!(
        "{}\n{table}\n\n",
        opts.bold("Projected per-entrypoint module count after a full codemod")
    )
}

fn skips_table(skips: &[(SkipReason, usize)], opts: &RenderOptions) -> String {
    if skips.is_empty() {
        return String::new();
    }
    let mut table = new_table();
    table.set_header(vec![header("reason", opts), header("symbols", opts)]);
    for (reason, count) in skips {
        table.add_row(vec![Cell::new(format!("{reason:?}")), Cell::new(*count)]);
    }
    format!(
        "{}\n{table}\n\n",
        opts.bold("Symbols not eligible for rewrite")
    )
}

fn format_ratio(value: f64) -> String {
    if value.is_infinite() {
        "∞".to_string()
    } else {
        format!("{value:.1}×")
    }
}

/// Focused output for the `cycles` subcommand.
pub fn render_cycles(analysis: &Analysis, cycles: &[Cycle], opts: &RenderOptions) -> String {
    let max_rows = if opts.max_rows == 0 {
        DEFAULT_MAX_ROWS
    } else {
        opts.max_rows
    };

    if cycles.is_empty() {
        return format!("{}\n", opts.bold("No cycles."));
    }

    let modules_in_cycles: usize = cycles.iter().map(Cycle::size).sum();
    let mut out = format!(
        "{}\n\n",
        opts.dim(&format!(
            "{} cycle(s) covering {modules_in_cycles} module(s)",
            cycles.len()
        ))
    );
    out.push_str(&cycles_section(
        &analysis.workspace.root,
        cycles,
        opts,
        max_rows,
    ));
    out
}

fn cycles_section(
    root: &std::path::Path,
    cycles: &[Cycle],
    opts: &RenderOptions,
    max_rows: usize,
) -> String {
    let mut out = format!("{}\n", opts.bold("Cycles"));

    for cycle in cycles.iter().take(max_rows) {
        let _ = writeln!(out, "  {} modules:", cycle.size());
        // A closed path reads better as a tree than as a comma-joined list.
        for (i, path) in cycle.shortest_path.iter().enumerate() {
            let display = relative(root, path).display().to_string();
            let branch = if i == 0 {
                "┌─".to_string()
            } else if i == cycle.shortest_path.len() - 1 {
                "└─".to_string()
            } else {
                "├─".to_string()
            };
            let _ = writeln!(out, "    {branch} {display}");
        }
        out.push('\n');
    }

    if cycles.len() > max_rows {
        let more = cycles.len() - max_rows;
        let _ = writeln!(
            out,
            "{}\n",
            opts.dim(&format!(
                "+{more} more, use --format json for the full list"
            ))
        );
    }
    out
}

fn fan_table(
    title: &str,
    root: &std::path::Path,
    entries: &[FanEntry],
    opts: &RenderOptions,
) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let mut table = new_table();
    table.set_header(vec![
        header("module", opts),
        header("direct", opts),
        header("transitive", opts),
    ]);
    for entry in entries {
        table.add_row(vec![
            Cell::new(relative(root, &entry.path).display().to_string()),
            Cell::new(entry.direct),
            Cell::new(entry.transitive),
        ]);
    }
    format!("{}\n{}\n\n", opts.bold(title), table)
}

fn summary_table(analysis: &Analysis, opts: &RenderOptions) -> String {
    let internal: usize = analysis
        .modules
        .iter()
        .map(|m| m.internal_deps().len())
        .sum();
    let externals = analysis.external_packages().len();

    let mut table = new_table();
    table.set_header(vec![header("metric", opts), header("value", opts)]);
    table.add_row(vec![
        Cell::new("files analyzed"),
        Cell::new(analysis.modules.len()),
    ]);
    table.add_row(vec![
        Cell::new("packages"),
        Cell::new(analysis.workspace.packages.len()),
    ]);
    table.add_row(vec![Cell::new("internal edges"), Cell::new(internal)]);
    table.add_row(vec![Cell::new("external packages"), Cell::new(externals)]);
    table.add_row(vec![
        Cell::new("unresolved specifiers"),
        Cell::new(analysis.unresolved.len()),
    ]);
    format!("{table}\n")
}

fn unresolved_table(analysis: &Analysis, opts: &RenderOptions, max_rows: usize) -> String {
    let root = &analysis.workspace.root;
    let mut table = new_table();
    table.set_header(vec![
        header("specifier", opts),
        header("imported from", opts),
        header("reason", opts),
    ]);

    for u in analysis.unresolved.iter().take(max_rows) {
        table.add_row(vec![
            Cell::new(&u.specifier),
            Cell::new(relative(root, &u.importer).display().to_string()),
            Cell::new(&u.reason),
        ]);
    }

    let mut out = format!("{}\n{}\n", opts.bold("Unresolved specifiers"), table);
    if analysis.unresolved.len() > max_rows {
        let more = analysis.unresolved.len() - max_rows;
        let _ = writeln!(
            out,
            "{}",
            opts.dim(&format!(
                "+{more} more, use --format json for the full list"
            ))
        );
    }
    out
}

fn parse_failure_table(
    analysis: &Analysis,
    failures: &[(&std::path::Path, &str)],
    opts: &RenderOptions,
    max_rows: usize,
) -> String {
    let root = &analysis.workspace.root;
    let mut table = new_table();
    table.set_header(vec![header("file", opts), header("first error", opts)]);

    for (path, error) in failures.iter().take(max_rows) {
        table.add_row(vec![
            Cell::new(relative(root, path).display().to_string()),
            Cell::new(error),
        ]);
    }

    let mut out = format!("{}\n{}\n", opts.bold("Files with parse errors"), table);
    if failures.len() > max_rows {
        let more = failures.len() - max_rows;
        let _ = writeln!(out, "{}", opts.dim(&format!("+{more} more")));
    }
    out
}

fn new_table() -> Table {
    let mut table = Table::new();
    table
        .load_style(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}

fn header(text: &str, opts: &RenderOptions) -> Cell {
    if opts.color {
        Cell::new(text.bold().to_string())
    } else {
        Cell::new(text)
    }
}
