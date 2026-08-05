use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use dashmap::DashMap;
use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};

use crate::facts::ExportRecord;
use crate::graph::ModuleGraph;
use crate::pipeline::Module;
use crate::resolve::Resolved;

/// Where an exported name actually comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Resolution {
    /// The module that declares the symbol, and the name it is declared under.
    Definition { module: PathBuf, name: String },
    /// Two `export *` sources provide the same name; guessing would be wrong.
    Ambiguous { candidates: Vec<PathBuf> },
    /// Resolution re-entered a module already being resolved.
    Cyclic,
    /// The chain ends in `node_modules` or a builtin.
    External { package: String },
    /// The name is not exported by that module at all.
    NotFound,
}

impl Resolution {
    /// Only a concrete definition site can be rewritten to. Everything else is
    /// reported and skipped — §5.1 and §6 both depend on this being conservative.
    pub fn is_codemod_eligible(&self) -> bool {
        matches!(self, Resolution::Definition { .. })
    }

    pub fn definition_path(&self) -> Option<&PathBuf> {
        match self {
            Resolution::Definition { module, .. } => Some(module),
            _ => None,
        }
    }
}

/// Walks re-export chains to find where a symbol is really declared.
pub struct SymbolResolver<'a> {
    graph: &'a ModuleGraph,
    /// Per-module index of local name → the import it came from, so that
    /// `import { X } from './x'; export { X };` is followed rather than treated as a
    /// local declaration. This pattern is extremely common in hand-written barrels.
    imported_names: HashMap<NodeIndex, HashMap<String, (String, String)>>,
    /// Specifier → resolution, per module.
    specifier_targets: HashMap<NodeIndex, HashMap<String, Resolved>>,
    memo: DashMap<(NodeIndex, String), Resolution>,
}

impl<'a> SymbolResolver<'a> {
    pub fn new(graph: &'a ModuleGraph, modules: &[Module]) -> Self {
        let mut imported_names = HashMap::new();
        let mut specifier_targets = HashMap::new();

        for module in modules {
            let Some(idx) = graph.index_of(&module.facts.path) else {
                continue;
            };

            let mut locals = HashMap::new();
            for import in &module.facts.imports {
                for binding in &import.bindings {
                    // Namespace imports bind an object, not a single symbol, so they
                    // cannot be followed to one definition site.
                    if binding.imported != "*" {
                        locals.insert(
                            binding.local.clone(),
                            (import.specifier.clone(), binding.imported.clone()),
                        );
                    }
                }
            }
            imported_names.insert(idx, locals);
            specifier_targets.insert(idx, module.resolutions.clone().into_iter().collect());
        }

        Self {
            graph,
            imported_names,
            specifier_targets,
            memo: DashMap::new(),
        }
    }

    /// Resolve `name` as exported by `module`.
    pub fn resolve(&self, module: NodeIndex, name: &str) -> Resolution {
        let mut visiting = HashSet::new();
        self.resolve_inner(module, name, &mut visiting)
    }

    fn resolve_inner(
        &self,
        module: NodeIndex,
        name: &str,
        visiting: &mut HashSet<(NodeIndex, String)>,
    ) -> Resolution {
        let key = (module, name.to_string());

        if let Some(hit) = self.memo.get(&key) {
            return hit.clone();
        }
        if !visiting.insert(key.clone()) {
            // Already resolving this exact symbol further up the stack.
            return Resolution::Cyclic;
        }

        let resolution = self.resolve_uncached(module, name, visiting);
        visiting.remove(&key);

        // A `Cyclic` verdict depends on the path taken to get here, so caching it
        // could poison an unrelated call site. Caching it is still *safe* — it only
        // ever makes the codemod skip more — but recomputing keeps reports precise.
        if resolution != Resolution::Cyclic {
            self.memo.insert(key, resolution.clone());
        }
        resolution
    }

    fn resolve_uncached(
        &self,
        module: NodeIndex,
        name: &str,
        visiting: &mut HashSet<(NodeIndex, String)>,
    ) -> Resolution {
        let node = self.graph.node(module);

        // A direct export of this name always wins over any `export *`.
        for export in &node.facts.exports {
            if export.exported_name() != Some(name) {
                continue;
            }
            return match export {
                ExportRecord::Local { .. } | ExportRecord::Default => {
                    self.resolve_local(module, name, visiting)
                }
                ExportRecord::Named { imported, from, .. } => {
                    self.follow(module, from, imported, visiting)
                }
                ExportRecord::NamespaceStar { from, .. } => {
                    // `export * as ns from './x'` defines the namespace object itself;
                    // the whole target module is the definition site.
                    match self.target_of(module, from) {
                        Some(Target::Internal(next)) => Resolution::Definition {
                            module: self.graph.path_of(next).to_path_buf(),
                            name: "*".to_string(),
                        },
                        Some(Target::External(package)) => Resolution::External { package },
                        None => Resolution::NotFound,
                    }
                }
                ExportRecord::Star { .. } => unreachable!("Star has no exported_name"),
            };
        }

        // Otherwise try each `export *` target, in source order.
        self.resolve_through_stars(module, name, visiting)
    }

    /// A name the module exports without a `from` clause. It may still have been
    /// imported at the top of the file, in which case that chain is followed.
    fn resolve_local(
        &self,
        module: NodeIndex,
        name: &str,
        visiting: &mut HashSet<(NodeIndex, String)>,
    ) -> Resolution {
        if let Some((specifier, imported)) = self
            .imported_names
            .get(&module)
            .and_then(|locals| locals.get(name))
        {
            return self.follow(module, specifier, imported, visiting);
        }
        Resolution::Definition {
            module: self.graph.path_of(module).to_path_buf(),
            name: name.to_string(),
        }
    }

    fn resolve_through_stars(
        &self,
        module: NodeIndex,
        name: &str,
        visiting: &mut HashSet<(NodeIndex, String)>,
    ) -> Resolution {
        let node = self.graph.node(module);
        let star_targets: Vec<String> = node
            .facts
            .exports
            .iter()
            .filter_map(|e| match e {
                ExportRecord::Star { from } => Some(from.clone()),
                _ => None,
            })
            .collect();

        let mut found: Vec<Resolution> = Vec::new();
        let mut saw_cycle = false;

        for from in &star_targets {
            let candidate = self.follow(module, from, name, visiting);
            match candidate {
                Resolution::NotFound => continue,
                Resolution::Cyclic => saw_cycle = true,
                other => {
                    // Two stars resolving to the *same* definition is not a conflict.
                    if !found.contains(&other) {
                        found.push(other);
                    }
                }
            }
        }

        match found.len() {
            0 if saw_cycle => Resolution::Cyclic,
            0 => Resolution::NotFound,
            1 => found.remove(0),
            _ => {
                let mut candidates: Vec<PathBuf> = found
                    .iter()
                    .filter_map(|r| r.definition_path().cloned())
                    .collect();
                candidates.sort();
                Resolution::Ambiguous { candidates }
            }
        }
    }

    /// Follow `specifier` out of `module` and resolve `name` on the other side.
    fn follow(
        &self,
        module: NodeIndex,
        specifier: &str,
        name: &str,
        visiting: &mut HashSet<(NodeIndex, String)>,
    ) -> Resolution {
        match self.target_of(module, specifier) {
            Some(Target::Internal(next)) => self.resolve_inner(next, name, visiting),
            Some(Target::External(package)) => Resolution::External { package },
            None => Resolution::NotFound,
        }
    }

    fn target_of(&self, module: NodeIndex, specifier: &str) -> Option<Target> {
        match self.specifier_targets.get(&module)?.get(specifier)? {
            Resolved::Internal { path } => self.graph.index_of(path).map(Target::Internal),
            Resolved::External { package, .. } => Some(Target::External(package.clone())),
            Resolved::Builtin { name } => Some(Target::External(name.clone())),
            Resolved::Unresolved { .. } => None,
        }
    }

    /// Every name a module exports, with where each one resolves to.
    pub fn resolve_all_exports(&self, module: NodeIndex) -> Vec<(String, Resolution)> {
        let node = self.graph.node(module);
        let mut names: Vec<String> = node
            .facts
            .exports
            .iter()
            .filter_map(|e| e.exported_name().map(str::to_string))
            .collect();
        names.sort();
        names.dedup();

        names
            .into_iter()
            .map(|name| {
                let resolution = self.resolve(module, &name);
                (name, resolution)
            })
            .collect()
    }
}

enum Target {
    Internal(NodeIndex),
    External(String),
}
