//! Mermaid flowchart renderer.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use unstave_core::graph::EdgeKind;

use crate::visualization::{project, VisualNode};
use crate::AnalysisReport;

/// Render a directory-clustered Mermaid flowchart.
pub fn render(report: &AnalysisReport, max_nodes: usize) -> String {
    let projected = project(report, max_nodes);
    let mut out = String::from("flowchart LR\n");
    if projected.collapsed {
        out.push_str("  %% Module count exceeded --max-nodes; nodes are directory aggregates.\n");
    }

    let mut clusters: BTreeMap<&str, Vec<&VisualNode>> = BTreeMap::new();
    for node in &projected.nodes {
        clusters.entry(&node.directory).or_default().push(node);
    }
    for (cluster_index, (directory, nodes)) in clusters.into_iter().enumerate() {
        let _ = writeln!(
            out,
            "  subgraph c{cluster_index}[\"{}\"]",
            escape(directory)
        );
        for node in nodes {
            let _ = writeln!(out, "    {}[\"{}\"]", node.id, escape(&node.label));
        }
        out.push_str("  end\n");
    }

    for edge in &projected.edges {
        let arrow = match edge.kind {
            EdgeKind::ReExport => "==>",
            EdgeKind::Dynamic | EdgeKind::TypeOnly => "-.->",
            EdgeKind::Static | EdgeKind::SideEffectOnly => "-->",
        };
        let _ = writeln!(
            out,
            "  {} {arrow}|\"{}\"| {}",
            edge.source,
            edge_label(edge.kind),
            edge.target,
        );
    }

    out.push_str("  classDef barrel fill:#3b2f16,stroke:#f59e0b,stroke-width:2px,color:#f8fafc;\n");
    out.push_str("  classDef cycle fill:#3f1d28,stroke:#f87171,stroke-width:3px,color:#f8fafc;\n");
    for node in &projected.nodes {
        if node.is_barrel {
            let _ = writeln!(out, "  class {} barrel;", node.id);
        }
        if node.in_cycle {
            let _ = writeln!(out, "  class {} cycle;", node.id);
        }
    }
    for (index, edge) in projected.edges.iter().enumerate() {
        let style = match edge.kind {
            EdgeKind::Static => "stroke:#94a3b8,stroke-width:1px",
            EdgeKind::Dynamic => "stroke:#60a5fa,stroke-width:1px,stroke-dasharray:5 5",
            EdgeKind::TypeOnly => "stroke:#64748b,stroke-width:1px,stroke-dasharray:2 4",
            EdgeKind::ReExport => "stroke:#f59e0b,stroke-width:2px",
            EdgeKind::SideEffectOnly => "stroke:#f87171,stroke-width:2px",
        };
        let _ = writeln!(out, "  linkStyle {index} {style};");
    }
    out
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
    text.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\n', " ")
}
