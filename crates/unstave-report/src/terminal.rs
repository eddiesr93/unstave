use std::fmt::Write as _;

use comfy_table::{presets::UTF8_FULL, Cell, ContentArrangement, Table};
use owo_colors::OwoColorize;
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
