use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;

use crate::config::{Config, ALWAYS_EXCLUDE_DIRS};
use crate::error::{Error, Result};

/// One npm package inside the analyzed workspace. Each gets its own resolver
/// context because `tsconfig` `paths` and `package.json` `exports` are per-package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// Directory containing the `package.json`.
    pub root: PathBuf,
    /// `name` from `package.json`, when it has one.
    pub name: Option<String>,
    /// Nearest `tsconfig.json` at the package root, if present.
    pub tsconfig: Option<PathBuf>,
    /// `sideEffects: false` in `package.json` means every module here is side-effect free.
    pub side_effects_false: bool,
}

/// How the workspace declares its packages. Recorded for reporting; package
/// discovery itself does not depend on it, because every `package.json` gets its own
/// resolver context regardless of whether a manifest lists it as a member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceKind {
    /// `pnpm-workspace.yaml` at the root.
    Pnpm,
    /// A `workspaces` field in the root `package.json` (npm, yarn, bun).
    NpmWorkspaces,
    /// A single package, or a layout we do not recognise.
    Single,
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub kind: WorkspaceKind,
    /// Always non-empty: the root itself is a package when nothing else is found.
    pub packages: Vec<Package>,
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

    let mut files = Vec::new();
    let mut package_jsons = Vec::new();

    for entry in builder.build() {
        let entry = entry.map_err(|source| Error::Walk {
            root: root.clone(),
            source,
        })?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.into_path();

        if path.file_name().is_some_and(|n| n == "package.json") {
            package_jsons.push(path);
            continue;
        }

        // Globs match on the workspace-relative path so patterns like `src/**/*.ts` work.
        let rel = path.strip_prefix(&root).unwrap_or(&path);
        if include.is_match(rel) && !exclude.is_match(rel) {
            files.push(path);
        }
    }

    // Deterministic order regardless of filesystem iteration order.
    files.sort();
    package_jsons.sort();

    let packages = build_packages(&root, &package_jsons);

    let kind = detect_workspace_kind(&root);

    Ok(Workspace {
        root,
        kind,
        packages,
        files,
    })
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

fn build_packages(root: &Path, package_jsons: &[PathBuf]) -> Vec<Package> {
    let mut packages: Vec<Package> = package_jsons
        .iter()
        .filter_map(|manifest| {
            let dir = manifest.parent()?;
            Some(read_package(dir, manifest))
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
                tsconfig: find_tsconfig(root),
                side_effects_false: false,
            },
        );
    }
    packages
}

fn read_package(dir: &Path, manifest: &Path) -> Package {
    let mut name = None;
    let mut side_effects_false = false;

    if let Ok(text) = std::fs::read_to_string(manifest) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            name = json
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            // Only a literal `false` means "nothing here has side effects". An array
            // or a glob string means some files do, which we treat conservatively.
            side_effects_false = json.get("sideEffects") == Some(&serde_json::Value::Bool(false));
        }
    }

    Package {
        root: dir.to_path_buf(),
        name,
        tsconfig: find_tsconfig(dir),
        side_effects_false,
    }
}

fn find_tsconfig(dir: &Path) -> Option<PathBuf> {
    ["tsconfig.json", "jsconfig.json"]
        .iter()
        .map(|f| dir.join(f))
        .find(|p| p.is_file())
}
