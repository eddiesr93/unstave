//! Why an import cannot be mechanically rewritten.

use serde::{Deserialize, Serialize};

/// Why an imported symbol or import statement cannot be rewritten to its
/// definition site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    /// Two `export *` sources provide the name.
    Ambiguous,
    /// The re-export chain loops.
    Cyclic,
    /// The chain ends outside the workspace.
    External,
    /// The name is not exported by the barrel.
    NotFound,
    /// `import * as ns from '...'` has no safe mechanical rewrite.
    NamespaceImport,
    /// The barrel itself has top-level side effects.
    BarrelHasSideEffects,
    /// The target file has an unresolved merge conflict.
    MergeConflict,
}

impl SkipReason {
    /// Stable, human-readable label used in CLI summaries.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ambiguous => "ambiguous symbol",
            Self::Cyclic => "cyclic re-export",
            Self::External => "external re-export",
            Self::NotFound => "symbol not found",
            Self::NamespaceImport => "namespace import",
            Self::BarrelHasSideEffects => "manual review — barrel has side effects",
            Self::MergeConflict => "merge conflict",
        }
    }
}
