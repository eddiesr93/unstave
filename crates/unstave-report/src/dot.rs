//! Graphviz DOT renderer.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use unstave_core::graph::EdgeKind;

use crate::visualization::{project, VisualNode};
use crate::AnalysisReport;

/// Render a directory-clustered graph, aggregating directories when needed.
pub fn render(report: &AnalysisReport, max_nodes: usize) -> String {
    let projected = project(report, max_nodes);
    let mut out = String::from(
        "digraph unstave {\n  graph [rankdir=LR, bgcolor=\"#111827\", fontcolor=\"#e5e7eb\", compound=true];\n  node [shape=ellipse, style=filled, fillcolor=\"#1f2937\", color=\"#64748b\", fontcolor=\"#e5e7eb\", fontname=\"Helvetica\"];\n  edge [color=\"#64748b\", fontcolor=\"#94a3b8\", fontname=\"Helvetica\", fontsize=9];\n",
    );

    if projected.collapsed {
        out.push_str("  // Module count exceeded --max-nodes; nodes are directory aggregates.\n");
    }

    let mut clusters: BTreeMap<&str, Vec<&VisualNode>> = BTreeMap::new();
    for node in &projected.nodes {
        clusters.entry(&node.directory).or_default().push(node);
    }

    for (cluster_index, (directory, nodes)) in clusters.into_iter().enumerate() {
        let _ = writeln!(out, "  subgraph cluster_{cluster_index} {{");
        let _ = writeln!(out, "    label=\"{}\";", escape(directory));
        out.push_str("    color=\"#334155\"; fontcolor=\"#94a3b8\";\n");
        for node in nodes {
            let shape = if node.is_barrel { "box" } else { "ellipse" };
            let (color, penwidth) = if node.in_cycle {
                ("#f87171", 2)
            } else if node.is_barrel {
                ("#f59e0b", 2)
            } else {
                ("#64748b", 1)
            };
            let _ = writeln!(
                out,
                "    {} [label=\"{}\", shape={shape}, color=\"{color}\", penwidth={penwidth}, tooltip=\"{} module(s)\"];",
                node.id,
                escape(&node.label),
                node.module_count,
            );
        }
        out.push_str("  }\n");
    }

    for edge in &projected.edges {
        let (style, color, penwidth) = edge_style(edge.kind);
        let _ = writeln!(
            out,
            "  {} -> {} [label=\"{}\", style={style}, color=\"{color}\", penwidth={penwidth}];",
            edge.source,
            edge.target,
            edge_label(edge.kind),
        );
    }
    out.push_str("}\n");
    out
}

fn edge_style(kind: EdgeKind) -> (&'static str, &'static str, usize) {
    match kind {
        EdgeKind::Static => ("solid", "#94a3b8", 1),
        EdgeKind::Dynamic => ("dashed", "#60a5fa", 1),
        EdgeKind::TypeOnly => ("dotted", "#64748b", 1),
        EdgeKind::ReExport => ("bold", "#f59e0b", 2),
        EdgeKind::SideEffectOnly => ("solid", "#f87171", 2),
    }
}

fn edge_label(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Static => "static",
        EdgeKind::Dynamic => "dynamic",
        EdgeKind::TypeOnly => "type-only",
        EdgeKind::ReExport => "re-export",
        EdgeKind::SideEffectOnly => "side-effect",
    }
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
