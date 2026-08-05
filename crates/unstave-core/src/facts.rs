use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Byte range into the original source. Mirrors `oxc_span::Span` but is owned,
/// serializable, and free of any oxc lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }
}

impl From<oxc_span::Span> for Span {
    fn from(s: oxc_span::Span) -> Self {
        Span::new(s.start, s.end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportKind {
    Named,
    Default,
    Namespace,
    SideEffect,
    Dynamic,
}

/// One imported name. `imported` is the name in the source module,
/// `local` the name it is bound to here. For a default import,
/// `imported` is `"default"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub local: String,
    pub imported: String,
    /// `import { type X }` — an inline type specifier.
    pub type_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRecord {
    pub specifier: String,
    pub kind: ImportKind,
    /// `import type { ... }` — the whole statement is type-only.
    pub type_only: bool,
    pub bindings: Vec<Binding>,
    /// Byte range of the whole statement.
    pub span: Span,
}

impl ImportRecord {
    /// True when nothing this statement pulls in survives to runtime.
    pub fn is_type_only(&self) -> bool {
        self.type_only || (!self.bindings.is_empty() && self.bindings.iter().all(|b| b.type_only))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExportRecord {
    /// Declared in this module.
    Local {
        name: String,
        type_only: bool,
    },
    /// `export { imported as name } from "from"`.
    Named {
        name: String,
        imported: String,
        from: String,
        type_only: bool,
    },
    /// `export * from "./x"`.
    Star {
        from: String,
    },
    /// `export * as ns from "./x"`.
    NamespaceStar {
        name: String,
        from: String,
    },
    Default,
}

impl ExportRecord {
    /// The name this record makes available to importers, if it is a single known name.
    /// `Star` re-exports contribute an unknown set, so they have no name.
    pub fn exported_name(&self) -> Option<&str> {
        match self {
            ExportRecord::Local { name, .. } => Some(name),
            ExportRecord::Named { name, .. } => Some(name),
            ExportRecord::NamespaceStar { name, .. } => Some(name),
            ExportRecord::Default => Some("default"),
            ExportRecord::Star { .. } => None,
        }
    }

    /// True when this export forwards a symbol from another module.
    pub fn is_reexport(&self) -> bool {
        matches!(
            self,
            ExportRecord::Named { .. }
                | ExportRecord::Star { .. }
                | ExportRecord::NamespaceStar { .. }
        )
    }

    /// The specifier this export forwards through, if any.
    pub fn from_specifier(&self) -> Option<&str> {
        match self {
            ExportRecord::Named { from, .. }
            | ExportRecord::Star { from }
            | ExportRecord::NamespaceStar { from, .. } => Some(from),
            _ => None,
        }
    }
}

/// Everything we extract from one source file. Owned — the oxc arena is gone by the
/// time this leaves the parse closure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleFacts {
    pub path: PathBuf,
    pub content_hash: u64,
    pub imports: Vec<ImportRecord>,
    pub exports: Vec<ExportRecord>,
    /// Top-level statements beyond declarations, imports and exports —
    /// or a `package.json` `sideEffects` field that says so.
    pub has_side_effects: bool,
    /// Value + type declarations defined here.
    pub own_decl_count: usize,
    /// Parse diagnostics, kept as strings; a file that fails to parse still gets a node.
    pub parse_errors: Vec<String>,
}

impl ModuleFacts {
    pub fn empty(path: PathBuf) -> Self {
        Self {
            path,
            content_hash: 0,
            imports: Vec::new(),
            exports: Vec::new(),
            has_side_effects: false,
            own_decl_count: 0,
            parse_errors: Vec::new(),
        }
    }

    /// Distinct specifiers this module depends on, in source order, deduplicated.
    pub fn specifiers(&self) -> Vec<&str> {
        let mut seen = Vec::new();
        for spec in self
            .imports
            .iter()
            .map(|i| i.specifier.as_str())
            .chain(self.exports.iter().filter_map(|e| e.from_specifier()))
        {
            if !seen.contains(&spec) {
                seen.push(spec);
            }
        }
        seen
    }
}
