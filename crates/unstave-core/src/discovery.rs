use std::path::{Path, PathBuf};
use std::sync::Mutex;

use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::{ParallelVisitor, ParallelVisitorBuilder, WalkBuilder, WalkState};

use crate::config::{Config, ALWAYS_EXCLUDE_DIRS};
use crate::error::{Error, Result};

/// One npm package inside the analyzed workspace. Each gets its own resolver
/// context because `tsconfig` `paths` and `package.json` `exports` are per-package.
#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct Package {
    /// Directory containing the `package.json`.
    #[rkyv(with = rkyv::with::AsString)]
    pub root: PathBuf,
    /// `name` from `package.json`, when it has one.
    pub name: Option<String>,
    /// Nearest `tsconfig.json` at the package root, if present.
    #[rkyv(with = rkyv::with::Map<rkyv::with::AsString>)]
    pub tsconfig: Option<PathBuf>,
    /// `sideEffects: false` in `package.json` means every module here is side-effect free.
    pub side_effects_false: bool,
    /// Source files exposed through the package's public `exports` map.
    ///
    /// Dead-export analysis treats these and everything reachable from them as
    /// public API. Wildcard targets conservatively expand to every matching source
    /// file discovered inside the package.
    #[rkyv(with = rkyv::with::Map<rkyv::with::AsString>)]
    pub public_entrypoints: Vec<PathBuf>,
}

/// How the workspace declares its packages. Recorded for reporting; package
/// discovery itself does not depend on it, because every `package.json` gets its own
/// resolver context regardless of whether a manifest lists it as a member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum WorkspaceKind {
    /// `pnpm-workspace.yaml` at the root.
    Pnpm,
    /// A `workspaces` field in the root `package.json` (npm, yarn, bun).
    NpmWorkspaces,
    /// A single package, or a layout we do not recognise.
    Single,
}

#[derive(Debug, Clone, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct Workspace {
    #[rkyv(with = rkyv::with::AsString)]
    pub root: PathBuf,
    pub kind: WorkspaceKind,
    /// Always non-empty: the root itself is a package when nothing else is found.
    pub packages: Vec<Package>,
    #[rkyv(with = rkyv::with::Map<rkyv::with::AsString>)]
    pub files: Vec<PathBuf>,
}

impl Workspace {
    /// The package owning `path` — the one with the longest matching root.
    pub fn package_for(&self, path: &Path) -> &Package {
        self.packages
            .iter()
            .filter(|p| path.starts_with(&p.root))
            .max_by_key(|p| p.root.as_os_str().len())
            .unwrap_or(&self.packages[0])
    }
}

/// Walk `root`, honouring `.gitignore`, include/exclude globs, and the always-excluded
/// build directories.
pub fn discover(root: &Path, config: &Config) -> Result<Workspace> {
    let root = root
        .canonicalize()
        .map_err(|e| Error::io(root.to_path_buf(), e))?;

    let include = build_globset(&config.include_globs())?;
    let exclude = build_globset(&config.exclude)?;

    let mut builder = WalkBuilder::new(&root);
    builder
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .parents(true)
        .follow_links(false);

    // `.gitignore` usually covers these, but not every repo ignores `dist` or `build`,
    // and walking `node_modules` is the single biggest cost we can avoid.
    builder.filter_entry(|entry| {
        if entry.file_type().is_some_and(|t| t.is_dir()) {
            if let Some(name) = entry.file_name().to_str() {
                return !ALWAYS_EXCLUDE_DIRS.contains(&name);
            }
        }
        true
    });

    // Walking a deep workspace is dominated by directory reads, so it runs across
    // threads. Each visitor accumulates locally and merges once when it is dropped:
    // a per-file channel send costs more than the walk saves on shallow trees.
    let collected = Mutex::new(Collected::default());
    let mut visitors = VisitorBuilder {
        root: &root,
        include: &include,
        exclude: &exclude,
        collected: &collected,
    };
    builder.build_parallel().visit(&mut visitors);

    let Collected {
        mut files,
        mut package_jsons,
        error,
    } = collected.into_inner().unwrap_or_default();

    if let Some(source) = error {
        return Err(Error::Walk {
            root: root.clone(),
            source,
        });
    }

    // Deterministic order regardless of filesystem iteration order.
    files.sort();
    package_jsons.sort();

    let packages = build_packages(&root, &package_jsons, &files);

    let kind = detect_workspace_kind(&root);

    Ok(Workspace {
        root,
        kind,
        packages,
        files,
    })
}

/// What the parallel walk produces, merged from every visitor.
#[derive(Default)]
struct Collected {
    files: Vec<PathBuf>,
    package_jsons: Vec<PathBuf>,
    error: Option<ignore::Error>,
}

struct VisitorBuilder<'a> {
    root: &'a Path,
    include: &'a GlobSet,
    exclude: &'a GlobSet,
    collected: &'a Mutex<Collected>,
}

impl<'a, 's> ParallelVisitorBuilder<'s> for VisitorBuilder<'a>
where
    'a: 's,
{
    fn build(&mut self) -> Box<dyn ParallelVisitor + 's> {
        Box::new(Visitor {
            root: self.root,
            include: self.include,
            exclude: self.exclude,
            collected: self.collected,
            files: Vec::new(),
            package_jsons: Vec::new(),
            error: None,
        })
    }
}

struct Visitor<'a> {
    root: &'a Path,
    include: &'a GlobSet,
    exclude: &'a GlobSet,
    collected: &'a Mutex<Collected>,
    files: Vec<PathBuf>,
    package_jsons: Vec<PathBuf>,
    error: Option<ignore::Error>,
}

impl ParallelVisitor for Visitor<'_> {
    fn visit(&mut self, result: std::result::Result<ignore::DirEntry, ignore::Error>) -> WalkState {
        let entry = match result {
            Ok(entry) => entry,
            Err(source) => {
                self.error.get_or_insert(source);
                return WalkState::Quit;
            }
        };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            return WalkState::Continue;
        }
        let path = entry.into_path();

        if path.file_name().is_some_and(|n| n == "package.json") {
            self.package_jsons.push(path);
            return WalkState::Continue;
        }

        // Globs match on the workspace-relative path so patterns like `src/**/*.ts` work.
        let rel = path.strip_prefix(self.root).unwrap_or(&path);
        if self.include.is_match(rel) && !self.exclude.is_match(rel) {
            self.files.push(path);
        }
        WalkState::Continue
    }
}

impl Drop for Visitor<'_> {
    fn drop(&mut self) {
        let Ok(mut collected) = self.collected.lock() else {
            return;
        };
        collected.files.append(&mut self.files);
        collected.package_jsons.append(&mut self.package_jsons);
        if let Some(error) = self.error.take() {
            collected.error.get_or_insert(error);
        }
    }
}

/// Recognise the workspace layout from its root manifests.
///
/// Deliberately does not parse the member globs: they would only ever *narrow* the
/// package set, and a package with its own `tsconfig`/`exports` needs its own
/// resolver whether or not a root manifest lists it. Reading the globs would mean
/// taking a YAML dependency to gain nothing.
fn detect_workspace_kind(root: &Path) -> WorkspaceKind {
    if root.join("pnpm-workspace.yaml").is_file() || root.join("pnpm-workspace.yml").is_file() {
        return WorkspaceKind::Pnpm;
    }
    if let Ok(text) = std::fs::read_to_string(root.join("package.json")) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            if json.get("workspaces").is_some() {
                return WorkspaceKind::NpmWorkspaces;
            }
        }
    }
    WorkspaceKind::Single
}

fn build_globset(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|source| Error::Glob {
            pattern: pattern.clone(),
            source,
        })?;
        builder.add(glob);
    }
    builder.build().map_err(|source| Error::Glob {
        pattern: patterns.join(", "),
        source,
    })
}

fn build_packages(root: &Path, package_jsons: &[PathBuf], files: &[PathBuf]) -> Vec<Package> {
    let mut packages: Vec<Package> = package_jsons
        .iter()
        .filter_map(|manifest| {
            let dir = manifest.parent()?;
            Some(read_package(dir, manifest, files, root))
        })
        .collect();

    // The workspace root is always a package, even without a `package.json`, so that
    // `package_for` has something to fall back to.
    if !packages.iter().any(|p| p.root == root) {
        packages.insert(
            0,
            Package {
                root: root.to_path_buf(),
                name: None,
                tsconfig: find_tsconfig(root, root),
                side_effects_false: false,
                public_entrypoints: Vec::new(),
            },
        );
    }
    packages
}

fn read_package(dir: &Path, manifest: &Path, files: &[PathBuf], root: &Path) -> Package {
    let mut name = None;
    let mut side_effects_false = false;
    let mut public_entrypoints = Vec::new();

    if let Ok(text) = std::fs::read_to_string(manifest) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            name = json
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            // Only a literal `false` means "nothing here has side effects". An array
            // or a glob string means some files do, which we treat conservatively.
            side_effects_false = json.get("sideEffects") == Some(&serde_json::Value::Bool(false));
            if let Some(exports) = json.get("exports") {
                let mut targets = Vec::new();
                collect_export_targets(exports, &mut targets);
                for target in targets {
                    expand_export_target(dir, target, files, &mut public_entrypoints);
                }
            }
        }
    }

    public_entrypoints.sort();
    public_entrypoints.dedup();

    Package {
        root: dir.to_path_buf(),
        name,
        tsconfig: find_tsconfig(dir, root),
        side_effects_false,
        public_entrypoints,
    }
}

fn collect_export_targets<'a>(value: &'a serde_json::Value, targets: &mut Vec<&'a str>) {
    match value {
        serde_json::Value::String(target) if target.starts_with('.') => targets.push(target),
        serde_json::Value::Array(items) => {
            for item in items {
                collect_export_targets(item, targets);
            }
        }
        serde_json::Value::Object(map) => {
            for target in map.values() {
                collect_export_targets(target, targets);
            }
        }
        _ => {}
    }
}

fn expand_export_target(
    package_root: &Path,
    target: &str,
    files: &[PathBuf],
    out: &mut Vec<PathBuf>,
) {
    let relative = target.trim_start_matches("./");
    if let Some((prefix, _)) = relative.split_once('*') {
        let prefix = package_root.join(prefix);
        out.extend(
            files
                .iter()
                .filter(|file| file.starts_with(&prefix))
                .cloned(),
        );
        return;
    }

    let path = package_root.join(relative);
    if path.is_file() {
        out.push(path);
        return;
    }

    for extension in ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"] {
        let candidate = path.with_extension(extension);
        if candidate.is_file() {
            out.push(candidate);
            return;
        }
    }

    for file_name in [
        "index.ts",
        "index.tsx",
        "index.mts",
        "index.cts",
        "index.js",
        "index.jsx",
        "index.mjs",
        "index.cjs",
    ] {
        let candidate = path.join(file_name);
        if candidate.is_file() {
            out.push(candidate);
            return;
        }
    }
}

/// The tsconfig governing `dir`, searching upwards to `root`.
///
/// A package without its own `tsconfig.json` inherits the nearest ancestor's, which
/// is how `tsc` and every bundler behave. Stopping at the package directory instead
/// is a silent trap: an app folder that has a `package.json` but shares the
/// workspace's root tsconfig would get a resolver with no `paths` at all, so every
/// aliased import fails to resolve and every count behind it is understated rather
/// than reported missing.
fn find_tsconfig(dir: &Path, root: &Path) -> Option<PathBuf> {
    let mut current = Some(dir);
    while let Some(here) = current {
        if let Some(found) = ["tsconfig.json", "jsconfig.json"]
            .iter()
            .map(|f| here.join(f))
            .find(|p| p.is_file())
        {
            return Some(found);
        }
        if here == root {
            break;
        }
        current = here.parent();
    }
    None
}
