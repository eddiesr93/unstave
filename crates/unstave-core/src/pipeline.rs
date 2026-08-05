use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use oxc_allocator::Allocator;
use rayon::prelude::*;

use crate::config::Config;
use crate::discovery::{discover, Workspace};
use crate::error::Result;
use crate::facts::ModuleFacts;
use crate::parse::parse_module;
use crate::resolve::{Resolved, ResolverSet, UnresolvedSpecifier};

/// Facts plus resolution for one module.
#[derive(Debug, Clone)]
pub struct Module {
    pub facts: ModuleFacts,
    /// Every distinct specifier in this module, mapped to what it resolved to.
    pub resolutions: BTreeMap<String, Resolved>,
}

impl Module {
    pub fn path(&self) -> &Path {
        &self.facts.path
    }

    /// Internal targets this module depends on, deduplicated.
    pub fn internal_deps(&self) -> Vec<&Path> {
        let mut deps: Vec<&Path> = self
            .resolutions
            .values()
            .filter_map(Resolved::internal_path)
            .collect();
        deps.dedup();
        deps
    }
}

/// The result of the discovery + parse + resolve passes. Graph construction and the
/// analyses proper build on top of this.
pub struct Analysis {
    pub workspace: Workspace,
    pub modules: Vec<Module>,
    pub unresolved: Vec<UnresolvedSpecifier>,
    pub timings: Timings,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Timings {
    pub discovery_ms: u128,
    pub parse_ms: u128,
    pub resolve_ms: u128,
    pub total_ms: u128,
}

impl Analysis {
    /// Files that failed to parse, with their first diagnostic.
    pub fn parse_failures(&self) -> Vec<(&Path, &str)> {
        self.modules
            .iter()
            .filter_map(|m| {
                m.facts
                    .parse_errors
                    .first()
                    .map(|e| (m.facts.path.as_path(), e.as_str()))
            })
            .collect()
    }

    pub fn external_packages(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .modules
            .iter()
            .flat_map(|m| m.resolutions.values())
            .filter_map(|r| match r {
                Resolved::External { package, .. } => Some(package.as_str()),
                _ => None,
            })
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }
}

/// Run discovery, parsing and resolution over `root`.
pub fn analyze(root: &Path, config: &Config) -> Result<Analysis> {
    let started = Instant::now();

    let t = Instant::now();
    let workspace = discover(root, config)?;
    let discovery_ms = t.elapsed().as_millis();

    // Parse in parallel. The allocator is not `Send`, so each closure builds its own
    // and everything it borrows is dropped before the owned facts are returned.
    let t = Instant::now();
    let facts: Vec<ModuleFacts> = workspace
        .files
        .par_iter()
        .map(|path| {
            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                // An unreadable file is a diagnostic, not a reason to abort the run.
                Err(e) => {
                    let mut facts = ModuleFacts::empty(path.clone());
                    facts.parse_errors.push(format!("could not read file: {e}"));
                    return facts;
                }
            };
            let allocator = Allocator::default();
            parse_module(path, &source, &allocator)
        })
        .collect();
    let parse_ms = t.elapsed().as_millis();

    let t = Instant::now();
    let resolvers = ResolverSet::new(&workspace);
    let resolved: Vec<(Module, Vec<UnresolvedSpecifier>)> = facts
        .into_par_iter()
        .map(|facts| resolve_module(&workspace, &resolvers, facts))
        .collect();
    let resolve_ms = t.elapsed().as_millis();

    let mut modules = Vec::with_capacity(resolved.len());
    let mut unresolved = Vec::new();
    for (module, mut misses) in resolved {
        modules.push(module);
        unresolved.append(&mut misses);
    }
    unresolved.sort_by(|a, b| {
        a.importer
            .cmp(&b.importer)
            .then(a.span.start.cmp(&b.span.start))
    });

    Ok(Analysis {
        workspace,
        modules,
        unresolved,
        timings: Timings {
            discovery_ms,
            parse_ms,
            resolve_ms,
            total_ms: started.elapsed().as_millis(),
        },
    })
}

fn resolve_module(
    workspace: &Workspace,
    resolvers: &ResolverSet,
    facts: ModuleFacts,
) -> (Module, Vec<UnresolvedSpecifier>) {
    let package = workspace.package_for(&facts.path);
    let mut resolutions = BTreeMap::new();
    let mut misses = Vec::new();

    for specifier in facts.specifiers() {
        let resolved = resolvers.resolve(package, &facts.path, specifier);
        if let Resolved::Unresolved { reason } = &resolved {
            misses.push(UnresolvedSpecifier {
                specifier: specifier.to_string(),
                importer: facts.path.clone(),
                span: span_of(&facts, specifier),
                reason: reason.clone(),
            });
        }
        resolutions.insert(specifier.to_string(), resolved);
    }

    (Module { facts, resolutions }, misses)
}

/// Where a specifier appears in its importer. Exports carry no span of their own yet,
/// so a re-export-only specifier reports the start of the file.
fn span_of(facts: &ModuleFacts, specifier: &str) -> crate::facts::Span {
    facts
        .imports
        .iter()
        .find(|i| i.specifier == specifier)
        .map(|i| i.span)
        .unwrap_or(crate::facts::Span::new(0, 0))
}

/// Convenience for callers that only have a root path.
pub fn analyze_root(root: impl AsRef<Path>) -> Result<Analysis> {
    let root = root.as_ref();
    let config = Config::load(root, None)?;
    analyze(root, &config)
}

/// Workspace-relative display path, falling back to the absolute path.
pub fn relative(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}
