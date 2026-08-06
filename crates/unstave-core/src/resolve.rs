use std::collections::HashMap;
use std::path::{Path, PathBuf};

use oxc_resolver::{
    EnforceExtension, ResolveOptions, Resolver, TsconfigDiscovery, TsconfigOptions,
    TsconfigReferences,
};
use serde::{Deserialize, Serialize};

use crate::discovery::{Package, Workspace};
use crate::facts::Span;

/// Node builtins that carry no `node:` prefix. Not exhaustive by design — anything
/// missing simply falls through to a normal resolve attempt.
const NODE_BUILTINS: &[&str] = &[
    "assert",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "dns",
    "domain",
    "events",
    "fs",
    "http",
    "http2",
    "https",
    "inspector",
    "module",
    "net",
    "os",
    "path",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "repl",
    "stream",
    "string_decoder",
    "timers",
    "tls",
    "trace_events",
    "tty",
    "url",
    "util",
    "v8",
    "vm",
    "worker_threads",
    "zlib",
];

/// What a specifier turned out to be. Only [`Internal`](Resolved::Internal) becomes a
/// traversable graph node; the rest are leaves or diagnostics.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Resolved {
    /// Inside the analyzed workspace.
    Internal {
        #[rkyv(with = rkyv::with::AsString)]
        path: PathBuf,
    },
    /// Resolves into `node_modules`.
    External {
        #[rkyv(with = rkyv::with::AsString)]
        path: PathBuf,
        package: String,
    },
    /// `node:` protocol or a known builtin.
    Builtin { name: String },
    /// Recorded, never fatal.
    Unresolved { reason: String },
}

impl Resolved {
    pub fn internal_path(&self) -> Option<&Path> {
        match self {
            Resolved::Internal { path } => Some(path),
            _ => None,
        }
    }
}

/// An unresolved specifier, with enough context to point at it in the source.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedSpecifier {
    pub specifier: String,
    #[rkyv(with = rkyv::with::AsString)]
    pub importer: PathBuf,
    pub span: Span,
    pub reason: String,
}

/// Holds one [`Resolver`] per package. Constructing a resolver is expensive, so they
/// are built once up front and shared across the parallel resolve pass.
pub struct ResolverSet {
    root: PathBuf,
    by_package: HashMap<PathBuf, Resolver>,
    fallback: Resolver,
}

impl ResolverSet {
    pub fn new(workspace: &Workspace) -> Self {
        let by_package = workspace
            .packages
            .iter()
            .map(|pkg| (pkg.root.clone(), build_resolver(pkg)))
            .collect();

        Self {
            root: workspace.root.clone(),
            by_package,
            fallback: Resolver::new(base_options()),
        }
    }

    fn resolver_for(&self, package: &Package) -> &Resolver {
        self.by_package.get(&package.root).unwrap_or(&self.fallback)
    }

    /// Resolve `specifier` as imported from `importer`, then classify the result.
    pub fn resolve(&self, package: &Package, importer: &Path, specifier: &str) -> Resolved {
        if let Some(name) = builtin_name(specifier) {
            return Resolved::Builtin { name };
        }

        let dir = match importer.parent() {
            Some(d) => d,
            None => {
                return Resolved::Unresolved {
                    reason: "importer has no parent directory".to_string(),
                }
            }
        };

        match self.resolver_for(package).resolve(dir, specifier) {
            Ok(resolution) => self.classify(resolution.full_path()),
            Err(err) => Resolved::Unresolved {
                reason: err.to_string(),
            },
        }
    }

    fn classify(&self, path: PathBuf) -> Resolved {
        if let Some(package) = node_modules_package(&path) {
            return Resolved::External { path, package };
        }
        // Compare in the same canonical form the graph uses to key its node index, so
        // a resolved path that discovery would spell slightly differently on Windows
        // (back/forward slashes, `\\?\` prefix, drive or component case, uncollapsed
        // `..`) still counts as inside the workspace. A raw `Path::starts_with` is
        // byte- and case-sensitive and would misclassify such a path as external,
        // silently dropping every edge into it before the graph index is even built.
        let key = crate::graph::module_path_key(&path);
        let root = crate::graph::module_path_key(&self.root);
        if key == root || key.starts_with(&format!("{root}/")) {
            Resolved::Internal { path }
        } else {
            // Resolved cleanly but landed outside the workspace — a linked package or
            // a path escaping the root. Treat it as external, named by its directory.
            let package = path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            Resolved::External { path, package }
        }
    }
}

fn base_options() -> ResolveOptions {
    ResolveOptions {
        extensions: [
            ".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs", ".json",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        // TypeScript source imports are written `./foo` or, under NodeNext, `./foo.js`
        // pointing at `./foo.ts`. Both need to land on the real file.
        extension_alias: vec![
            (
                ".js".into(),
                vec![".ts".into(), ".tsx".into(), ".js".into()],
            ),
            (".mjs".into(), vec![".mts".into(), ".mjs".into()]),
            (".cjs".into(), vec![".cts".into(), ".cjs".into()]),
        ],
        main_fields: ["module", "browser", "main"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        condition_names: ["import", "module", "browser", "default"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        enforce_extension: EnforceExtension::Disabled,
        prefer_relative: false,
        ..ResolveOptions::default()
    }
}

fn build_resolver(package: &Package) -> Resolver {
    let mut options = base_options();
    if let Some(tsconfig) = &package.tsconfig {
        // `baseUrl` and `paths` come from here; `references: Auto` follows project
        // references so a composite monorepo resolves across projects.
        options.tsconfig = Some(TsconfigDiscovery::Manual(TsconfigOptions {
            config_file: tsconfig.clone(),
            references: TsconfigReferences::Auto,
        }));
    }
    Resolver::new(options)
}

fn builtin_name(specifier: &str) -> Option<String> {
    if let Some(rest) = specifier.strip_prefix("node:") {
        return Some(rest.to_string());
    }
    if NODE_BUILTINS.contains(&specifier) {
        return Some(specifier.to_string());
    }
    None
}

/// The package name owning a path inside `node_modules`, handling `@scope/name`.
fn node_modules_package(path: &Path) -> Option<String> {
    let components: Vec<_> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    // Use the last `node_modules` segment: nested installs belong to the inner package.
    let idx = components.iter().rposition(|c| c == "node_modules")?;
    let first = components.get(idx + 1)?;
    if first.starts_with('@') {
        let second = components.get(idx + 2)?;
        Some(format!("{first}/{second}"))
    } else {
        Some(first.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_scoped_and_plain_package_names() {
        assert_eq!(
            node_modules_package(Path::new("/w/node_modules/react/index.js")),
            Some("react".to_string())
        );
        assert_eq!(
            node_modules_package(Path::new("/w/node_modules/@scope/pkg/dist/i.js")),
            Some("@scope/pkg".to_string())
        );
        // Nested installs resolve to the inner package, not the outer one.
        assert_eq!(
            node_modules_package(Path::new("/w/node_modules/a/node_modules/b/i.js")),
            Some("b".to_string())
        );
        assert_eq!(node_modules_package(Path::new("/w/src/a.ts")), None);
    }

    #[test]
    fn recognises_builtins_with_and_without_prefix() {
        assert_eq!(builtin_name("node:fs"), Some("fs".to_string()));
        assert_eq!(builtin_name("path"), Some("path".to_string()));
        assert_eq!(builtin_name("./path"), None);
        assert_eq!(builtin_name("react"), None);
    }
}
