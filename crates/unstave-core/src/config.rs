use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const CONFIG_FILE: &str = "unstave.toml";

/// Everything in `unstave.toml` is optional; CLI flags layer on top via
/// [`Config::load`] followed by the CLI's own flag handling.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub entrypoints: Vec<String>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub barrel: BarrelConfig,
    pub thresholds: Thresholds,
    pub codemod: CodemodConfig,
    pub resolve: ResolveConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResolveConfig {
    /// Extra export conditions to honour, on top of the defaults.
    ///
    /// Monorepos commonly point a custom condition at TypeScript source so that
    /// development resolves to `src/` while published builds resolve to `dist/`.
    /// TanStack Query uses `@tanstack/custom-condition`; `development` and `source`
    /// are also widespread. Without the right condition the resolver lands on build
    /// output that may not exist yet, and cross-package imports silently fail to
    /// resolve — which makes every downstream count wrong rather than merely absent.
    pub conditions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BarrelConfig {
    /// Fraction of a module's exports that must be re-exports for it to count as a barrel.
    pub reexport_ratio: f64,
    /// Maximum number of own declarations a barrel may still have.
    pub max_own_decls: usize,
}

impl Default for BarrelConfig {
    fn default() -> Self {
        Self {
            reexport_ratio: 0.8,
            max_own_decls: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Thresholds {
    /// Informational in v1 — reported, never enforced.
    pub max_amplification: f64,
    pub max_cycles: usize,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            max_amplification: 5.0,
            max_cycles: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CodemodConfig {
    pub import_style: ImportStyle,
}

impl Default for CodemodConfig {
    fn default() -> Self {
        Self {
            import_style: ImportStyle::Preserve,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportStyle {
    Alias,
    Relative,
    Preserve,
}

pub const DEFAULT_INCLUDE: &[&str] = &["**/*.{ts,tsx,js,jsx,mts,cts,mjs,cjs}"];

/// Directories that are never worth walking, `.gitignore` or not.
pub const ALWAYS_EXCLUDE_DIRS: &[&str] = &[
    "node_modules",
    "dist",
    "build",
    ".next",
    ".turbo",
    ".output",
    "out",
    "coverage",
    ".nyc_output",
    ".git",
    ".unstave",
];

impl Config {
    /// Load `unstave.toml` from `path`, or from `root/unstave.toml` when `path` is `None`.
    /// A missing file at the default location is not an error.
    pub fn load(root: &Path, path: Option<&Path>) -> Result<Self> {
        let (path, required) = match path {
            Some(p) => (p.to_path_buf(), true),
            None => (root.join(CONFIG_FILE), false),
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).map_err(|source| Error::Config { path, source }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !required => {
                Ok(Config::default())
            }
            Err(e) => Err(Error::io(path, e)),
        }
    }

    pub fn include_globs(&self) -> Vec<String> {
        if self.include.is_empty() {
            DEFAULT_INCLUDE.iter().map(|s| s.to_string()).collect()
        } else {
            self.include.clone()
        }
    }

    pub fn entrypoint_paths(&self, root: &Path) -> Vec<PathBuf> {
        self.entrypoints.iter().map(|e| root.join(e)).collect()
    }
}
