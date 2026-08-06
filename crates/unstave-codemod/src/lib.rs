//! Conservative, span-based rewriting of barrel imports.

use std::collections::{BTreeMap, HashMap};
use std::path::{Component, Path, PathBuf};

use globset::GlobMatcher;
use unstave_core::analysis::barrel;
use unstave_core::analysis::symbols::{Resolution, SymbolResolver};
use unstave_core::config::ImportStyle;
use unstave_core::facts::{Binding, Span};
use unstave_core::graph::ModuleGraph;
use unstave_core::{Analysis, Config};

/// Re-exported so `unstave_codemod::SkipReason` keeps resolving to the single
/// shared [`SkipReason`] defined in `unstave-core`.
pub use unstave_core::analysis::skip::SkipReason;

/// Scope and path-style policy for one codemod plan.
#[derive(Debug, Clone)]
pub struct CodemodOptions {
    pub import_style: ImportStyle,
    pub only: Option<String>,
    pub barrel: Option<PathBuf>,
}

impl Default for CodemodOptions {
    fn default() -> Self {
        Self {
            import_style: ImportStyle::Preserve,
            only: None,
            barrel: None,
        }
    }
}

/// One source file before and after its planned edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: PathBuf,
    pub original: String,
    pub rewritten: String,
    pub imports_rewritten: usize,
}

/// Count of skipped import statements for one reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkipCount {
    pub reason: SkipReason,
    pub imports: usize,
}

/// Complete dry-run plan. Callers decide whether to print, check, or write it.
#[derive(Debug, Clone, Default)]
pub struct CodemodPlan {
    pub files: Vec<FileChange>,
    pub imports_rewritten: usize,
    pub skipped: Vec<SkipCount>,
}

impl CodemodPlan {
    pub fn files_changed(&self) -> usize {
        self.files.len()
    }

    /// Unified diff for every changed file, in deterministic path order.
    pub fn unified_diff(&self, root: &Path) -> String {
        let mut files = self.files.iter().collect::<Vec<_>>();
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let mut output = String::new();
        for file in files {
            let relative = file.path.strip_prefix(root).unwrap_or(&file.path);
            let path = relative.to_string_lossy().replace('\\', "/");
            let before = format!("a/{path}");
            let after = format!("b/{path}");
            let diff = similar::TextDiff::from_lines(&file.original, &file.rewritten);
            output.push_str(
                &diff
                    .unified_diff()
                    .context_radius(3)
                    .header(&before, &after)
                    .to_string(),
            );
        }
        output
    }
}

/// Build a rewrite plan without modifying the filesystem.
pub fn plan(
    analysis: &Analysis,
    graph: &ModuleGraph,
    config: &Config,
    options: &CodemodOptions,
) -> Result<CodemodPlan, Error> {
    let only = compile_only(options.only.as_deref())?;
    let scoped_barrel = options.barrel.as_ref().map(|path| {
        if path.is_absolute() {
            normalize_path(path)
        } else {
            normalize_path(&analysis.workspace.root.join(path))
        }
    });
    let symbols = SymbolResolver::new(graph, &analysis.modules);
    let aliases = AliasResolver::new(analysis);
    let barrels = barrel::classify(graph, &config.barrel);
    let barrels_by_path: HashMap<_, _> = barrels
        .iter()
        .map(|barrel| (barrel.path.as_path(), barrel))
        .collect();
    let mut result = CodemodPlan::default();
    let mut skipped_counts = BTreeMap::new();

    for module in &analysis.modules {
        if only.as_ref().is_some_and(|matcher| {
            let relative = module
                .path()
                .strip_prefix(&analysis.workspace.root)
                .unwrap_or(module.path());
            !matcher.is_match(relative)
        }) {
            continue;
        }
        let mut candidates = Vec::new();

        for import in &module.facts.imports {
            let Some(unstave_core::Resolved::Internal { path: target }) =
                module.resolutions.get(&import.specifier)
            else {
                continue;
            };
            let Some(barrel) = barrels_by_path.get(target.as_path()) else {
                continue;
            };
            if scoped_barrel.as_ref().is_some_and(|scope| target != scope) {
                continue;
            }
            if barrel.has_side_effects {
                record_skip(&mut skipped_counts, SkipReason::BarrelHasSideEffects);
                continue;
            }
            if matches!(import.kind, unstave_core::ImportKind::Namespace)
                || import
                    .bindings
                    .iter()
                    .any(|binding| binding.imported == "*")
            {
                record_skip(&mut skipped_counts, SkipReason::NamespaceImport);
                continue;
            }
            let Some(target_node) = graph.index_of(target) else {
                continue;
            };

            let mut by_definition: BTreeMap<PathBuf, Vec<Binding>> = BTreeMap::new();
            let mut eligible = true;
            for binding in &import.bindings {
                let (definition, name) = match symbols.resolve(target_node, &binding.imported) {
                    Resolution::Definition { module, name } => (module, name),
                    Resolution::Ambiguous { .. } => {
                        record_skip(&mut skipped_counts, SkipReason::Ambiguous);
                        eligible = false;
                        break;
                    }
                    Resolution::Cyclic => {
                        record_skip(&mut skipped_counts, SkipReason::Cyclic);
                        eligible = false;
                        break;
                    }
                    Resolution::External { .. } => {
                        record_skip(&mut skipped_counts, SkipReason::External);
                        eligible = false;
                        break;
                    }
                    Resolution::NotFound => {
                        record_skip(&mut skipped_counts, SkipReason::NotFound);
                        eligible = false;
                        break;
                    }
                };
                by_definition.entry(definition).or_default().push(Binding {
                    local: binding.local.clone(),
                    imported: name,
                    type_only: binding.type_only,
                });
            }
            if !eligible || by_definition.is_empty() {
                continue;
            }

            candidates.push(Candidate {
                span: import.span,
                source_specifier: import.specifier.clone(),
                by_definition,
            });
        }

        candidates.retain(|candidate| {
            if has_namespace_merge_conflict(module, candidate) {
                record_skip(&mut skipped_counts, SkipReason::MergeConflict);
                false
            } else {
                true
            }
        });

        if candidates.is_empty() {
            continue;
        }

        let rewritten_imports = candidates.len();
        let mut targets: BTreeMap<PathBuf, TargetGroup> = BTreeMap::new();
        for candidate in &candidates {
            for (definition, bindings) in &candidate.by_definition {
                let target = targets
                    .entry(definition.clone())
                    .or_insert_with(|| TargetGroup {
                        source_specifier: candidate.source_specifier.clone(),
                        bindings: Vec::new(),
                    });
                extend_unique(&mut target.bindings, bindings);
            }
        }

        let candidate_spans = candidates
            .iter()
            .map(|candidate| candidate.span)
            .collect::<Vec<_>>();
        let mut existing: HashMap<PathBuf, Vec<&unstave_core::ImportRecord>> = HashMap::new();
        for import in &module.facts.imports {
            if candidate_spans.contains(&import.span)
                || matches!(import.kind, unstave_core::ImportKind::Dynamic)
            {
                continue;
            }
            let Some(unstave_core::Resolved::Internal { path }) =
                module.resolutions.get(&import.specifier)
            else {
                continue;
            };
            if targets.contains_key(path) {
                existing.entry(path.clone()).or_default().push(import);
            }
        }

        let mut edits = candidates
            .iter()
            .map(|candidate| Edit {
                span: candidate.span,
                replacement: String::new(),
            })
            .collect::<Vec<_>>();
        let mut unanchored = Vec::new();

        for (definition, target) in targets {
            if let Some(imports) = existing.get(&definition) {
                let mut combined = Vec::new();
                for import in imports {
                    extend_unique(&mut combined, &import.bindings);
                }
                extend_unique(&mut combined, &target.bindings);

                let anchor = imports[0];
                edits.push(Edit {
                    span: anchor.span,
                    replacement: render_import(&anchor.specifier, &combined, false),
                });
                for duplicate in imports.iter().skip(1) {
                    edits.push(Edit {
                        span: duplicate.span,
                        replacement: String::new(),
                    });
                }
                continue;
            }

            let specifier = definition_specifier(
                module,
                &definition,
                &target.source_specifier,
                options.import_style,
                &aliases,
            );
            let statement = render_import(&specifier, &target.bindings, false);
            unanchored.push((specifier, statement));
        }

        unanchored.sort_by(|left, right| left.0.cmp(&right.0));
        if !unanchored.is_empty() {
            if let Some(anchor) = edits
                .iter_mut()
                .filter(|edit| candidate_spans.contains(&edit.span))
                .min_by_key(|edit| edit.span.start)
            {
                anchor.replacement = unanchored
                    .into_iter()
                    .map(|(_, statement)| statement)
                    .collect::<Vec<_>>()
                    .join("\n");
            }
        }

        let original = std::fs::read_to_string(module.path()).map_err(|source| Error::Read {
            path: module.path().to_path_buf(),
            source,
        })?;
        let rewritten = apply_edits(&original, &mut edits);
        result.imports_rewritten += rewritten_imports;
        result.files.push(FileChange {
            path: module.path().to_path_buf(),
            original,
            rewritten,
            imports_rewritten: rewritten_imports,
        });
    }

    result.skipped = skipped_counts
        .into_iter()
        .map(|(reason, imports)| SkipCount { reason, imports })
        .collect();

    Ok(result)
}

fn record_skip(counts: &mut BTreeMap<SkipReason, usize>, reason: SkipReason) {
    *counts.entry(reason).or_default() += 1;
}

fn compile_only(pattern: Option<&str>) -> Result<Option<GlobMatcher>, Error> {
    pattern
        .map(|pattern| {
            globset::Glob::new(pattern)
                .map(|glob| glob.compile_matcher())
                .map_err(|source| Error::Glob {
                    pattern: pattern.to_string(),
                    source,
                })
        })
        .transpose()
}

fn has_namespace_merge_conflict(module: &unstave_core::Module, candidate: &Candidate) -> bool {
    module.facts.imports.iter().any(|import| {
        if import.span == candidate.span
            || !matches!(import.kind, unstave_core::ImportKind::Namespace)
        {
            return false;
        }
        matches!(
            module.resolutions.get(&import.specifier),
            Some(unstave_core::Resolved::Internal { path })
                if candidate.by_definition.contains_key(path)
        )
    })
}

#[derive(Debug)]
struct Candidate {
    span: Span,
    source_specifier: String,
    by_definition: BTreeMap<PathBuf, Vec<Binding>>,
}

#[derive(Debug)]
struct TargetGroup {
    source_specifier: String,
    bindings: Vec<Binding>,
}

#[derive(Debug)]
struct Edit {
    span: Span,
    replacement: String,
}

fn extend_unique(target: &mut Vec<Binding>, bindings: &[Binding]) {
    for binding in bindings {
        if !target.contains(binding) {
            target.push(binding.clone());
        }
    }
}

fn definition_specifier(
    module: &unstave_core::Module,
    definition: &Path,
    source_specifier: &str,
    style: ImportStyle,
    aliases: &AliasResolver,
) -> String {
    match style {
        ImportStyle::Alias => aliases
            .specifier_for(module.path(), definition)
            .unwrap_or_else(|| relative_specifier(module.path(), definition)),
        ImportStyle::Preserve
            if preserved_style(module, source_specifier) == ImportStyle::Alias =>
        {
            aliases
                .specifier_for(module.path(), definition)
                .unwrap_or_else(|| relative_specifier(module.path(), definition))
        }
        ImportStyle::Relative | ImportStyle::Preserve => {
            relative_specifier(module.path(), definition)
        }
    }
}

fn apply_edits(source: &str, edits: &mut [Edit]) -> String {
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.span.start));
    let mut output = source.to_string();
    for edit in edits {
        output.replace_range(
            edit.span.start as usize..edit.span.end as usize,
            &edit.replacement,
        );
    }
    output
}

fn render_import(specifier: &str, bindings: &[Binding], statement_type_only: bool) -> String {
    let all_type_only = statement_type_only || bindings.iter().all(|binding| binding.type_only);
    let default = bindings
        .iter()
        .find(|binding| binding.imported == "default");
    let can_use_default_clause = default.is_some_and(|binding| {
        (!binding.type_only && !statement_type_only) || (all_type_only && bindings.len() == 1)
    });
    let named = bindings
        .iter()
        .filter(|binding| !can_use_default_clause || binding.imported != "default")
        .map(|binding| {
            let prefix = if binding.type_only && !all_type_only {
                "type "
            } else {
                ""
            };
            if binding.imported == binding.local {
                format!("{prefix}{}", binding.imported)
            } else {
                format!("{prefix}{} as {}", binding.imported, binding.local)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    if can_use_default_clause {
        let default = default
            .map(|binding| binding.local.as_str())
            .unwrap_or("default");
        if named.is_empty() {
            let type_keyword = if all_type_only { " type" } else { "" };
            return format!("import{type_keyword} {default} from '{specifier}';");
        }
        return format!("import {default}, {{ {named} }} from '{specifier}';");
    }

    let type_keyword = if all_type_only { " type" } else { "" };
    format!("import{type_keyword} {{ {named} }} from '{specifier}';")
}

fn relative_specifier(importer: &Path, definition: &Path) -> String {
    let from = importer.parent().unwrap_or(importer);
    let from_components = normal_components(from);
    let target_components = normal_components(definition);
    let common = from_components
        .iter()
        .zip(&target_components)
        .take_while(|(left, right)| left == right)
        .count();

    let mut parts = Vec::new();
    parts.extend(std::iter::repeat_n(
        "..".to_string(),
        from_components.len() - common,
    ));
    parts.extend(
        target_components[common..]
            .iter()
            .map(|part| part.to_string_lossy().to_string()),
    );

    let joined = strip_module_extension(&parts.join("/"));
    if joined.starts_with('.') {
        joined
    } else {
        format!("./{joined}")
    }
}

fn normal_components(path: &Path) -> Vec<&std::ffi::OsStr> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part),
            _ => None,
        })
        .collect()
}

fn strip_module_extension(path: &str) -> String {
    const EXTENSIONS: &[&str] = &[
        ".d.ts", ".tsx", ".mts", ".cts", ".mjs", ".cjs", ".ts", ".jsx", ".js",
    ];
    EXTENSIONS
        .iter()
        .find_map(|extension| path.strip_suffix(extension).map(str::to_string))
        .unwrap_or_else(|| path.to_string())
}

fn preserved_style(module: &unstave_core::Module, current_specifier: &str) -> ImportStyle {
    let mut relative = 0;
    let mut alias = 0;
    for import in &module.facts.imports {
        if !matches!(
            module.resolutions.get(&import.specifier),
            Some(unstave_core::Resolved::Internal { .. })
        ) {
            continue;
        }
        if import.specifier.starts_with('.') {
            relative += 1;
        } else {
            alias += 1;
        }
    }
    match relative.cmp(&alias) {
        std::cmp::Ordering::Greater => ImportStyle::Relative,
        std::cmp::Ordering::Less => ImportStyle::Alias,
        std::cmp::Ordering::Equal if current_specifier.starts_with('.') => ImportStyle::Relative,
        std::cmp::Ordering::Equal => ImportStyle::Alias,
    }
}

#[derive(Debug)]
struct AliasResolver {
    by_package: HashMap<PathBuf, Vec<AliasRule>>,
}

impl AliasResolver {
    fn new(analysis: &Analysis) -> Self {
        let by_package = analysis
            .workspace
            .packages
            .iter()
            .map(|package| {
                let rules = package
                    .tsconfig
                    .as_deref()
                    .map(load_alias_rules)
                    .unwrap_or_default();
                (package.root.clone(), rules)
            })
            .collect();
        Self { by_package }
    }

    fn specifier_for(&self, importer: &Path, definition: &Path) -> Option<String> {
        let rules = self
            .by_package
            .iter()
            .filter(|(root, _)| importer.starts_with(root))
            .max_by_key(|(root, _)| root.as_os_str().len())?
            .1;
        let mut candidates = rules
            .iter()
            .filter_map(|rule| rule.apply(definition))
            .collect::<Vec<_>>();
        candidates
            .sort_by(|left, right| left.len().cmp(&right.len()).then_with(|| left.cmp(right)));
        candidates.into_iter().next()
    }
}

#[derive(Debug)]
struct AliasRule {
    alias: String,
    target: String,
}

impl AliasRule {
    fn apply(&self, definition: &Path) -> Option<String> {
        let definition = definition.to_string_lossy().replace('\\', "/");
        let captured = match self.target.split_once('*') {
            Some((prefix, suffix)) => definition
                .strip_prefix(prefix)?
                .strip_suffix(suffix)?
                .to_string(),
            None if definition == self.target => String::new(),
            None => return None,
        };
        let alias = if self.alias.contains('*') {
            self.alias.replace('*', &captured)
        } else if captured.is_empty() {
            self.alias.clone()
        } else {
            return None;
        };
        Some(strip_module_extension(&alias))
    }
}

fn load_alias_rules(tsconfig: &Path) -> Vec<AliasRule> {
    let Ok(mut text) = std::fs::read_to_string(tsconfig) else {
        return Vec::new();
    };
    if json_strip_comments::strip_comments_in_place(&mut text).is_err() {
        return Vec::new();
    }
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let Some(compiler) = json.get("compilerOptions") else {
        return Vec::new();
    };
    let config_dir = tsconfig.parent().unwrap_or_else(|| Path::new("."));
    let base = compiler
        .get("baseUrl")
        .and_then(serde_json::Value::as_str)
        .map_or_else(|| config_dir.to_path_buf(), |path| config_dir.join(path));
    let Some(paths) = compiler.get("paths").and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };

    let mut rules = Vec::new();
    for (alias, targets) in paths {
        let Some(targets) = targets.as_array() else {
            continue;
        };
        for target in targets.iter().filter_map(serde_json::Value::as_str) {
            rules.push(AliasRule {
                alias: alias.clone(),
                target: normalize_path(&base.join(target))
                    .to_string_lossy()
                    .replace('\\', "/"),
            });
        }
    }
    rules
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid --only glob `{pattern}`: {source}")]
    Glob {
        pattern: String,
        #[source]
        source: globset::Error,
    },
}
