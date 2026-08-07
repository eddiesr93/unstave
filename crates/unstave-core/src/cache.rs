//! Content-addressed analysis cache.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;
use xxhash_rust::xxh3::{xxh3_64, Xxh3};

use crate::config::{Config, ALWAYS_EXCLUDE_DIRS};
use crate::discovery::{discover, Workspace};
use crate::pipeline::{analyze_discovered, Analysis, Module, Timings};
use crate::resolve::UnresolvedSpecifier;
use crate::{Error, Result};

const CACHE_SCHEMA: u32 = 1;
const CACHE_FILE: &str = "cache-v1.rkyv";

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
struct CacheFile {
    schema: u32,
    fingerprint: u64,
    workspace: Workspace,
    modules: Vec<Module>,
    unresolved: Vec<UnresolvedSpecifier>,
}

/// Analyze a workspace, restoring parse and resolution results when every content
/// hash and configuration input still matches.
pub fn analyze_cached(root: &Path, config: &Config) -> Result<Analysis> {
    let started = Instant::now();
    let discovery_started = Instant::now();
    let workspace = discover(root, config)?;
    let discovery_ms = discovery_started.elapsed().as_millis();
    let fingerprint = workspace_fingerprint(&workspace, config);
    let path = cache_path(&workspace.root);

    if let Some(cache) = load(&path, fingerprint) {
        return Ok(Analysis {
            workspace: cache.workspace,
            modules: cache.modules,
            unresolved: cache.unresolved,
            timings: Timings {
                discovery_ms,
                parse_ms: 0,
                resolve_ms: 0,
                total_ms: started.elapsed().as_millis(),
            },
            cache_hit: true,
        });
    }

    let analysis = analyze_discovered(workspace, config, started, discovery_ms)?;
    store(&path, fingerprint, &analysis);
    Ok(analysis)
}

/// Stable location of the cache file for a workspace root.
pub fn cache_path(root: &Path) -> PathBuf {
    root.join(".unstave").join(CACHE_FILE)
}

/// Remove the exact cache file. Returns `false` when no cache existed.
pub fn clear_cache(root: &Path) -> Result<bool> {
    let path = cache_path(root);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(Error::io(path, source)),
    }
}

fn load(path: &Path, fingerprint: u64) -> Option<CacheFile> {
    let bytes = std::fs::read(path).ok()?;
    let cache = rkyv::from_bytes::<CacheFile, rkyv::rancor::Error>(&bytes).ok()?;
    (cache.schema == CACHE_SCHEMA && cache.fingerprint == fingerprint).then_some(cache)
}

fn store(path: &Path, fingerprint: u64, analysis: &Analysis) {
    let cache = CacheFile {
        schema: CACHE_SCHEMA,
        fingerprint,
        workspace: analysis.workspace.clone(),
        modules: analysis.modules.clone(),
        unresolved: analysis.unresolved.clone(),
    };
    let Ok(bytes) = rkyv::to_bytes::<rkyv::rancor::Error>(&cache) else {
        return;
    };
    let Some(directory) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(directory).is_ok() {
        let _ = std::fs::write(path, bytes);
    }
}

fn workspace_fingerprint(workspace: &Workspace, config: &Config) -> u64 {
    let source_hashes = workspace
        .files
        .par_iter()
        .map(|path| (path, content_hash(path)))
        .collect::<Vec<_>>();
    let mut hasher = Xxh3::new();
    hasher.update(&CACHE_SCHEMA.to_le_bytes());
    hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
    if let Ok(config) = serde_json::to_vec(config) {
        update_field(&mut hasher, b"config", &config);
    }
    for (path, hash) in source_hashes {
        update_path(&mut hasher, path);
        hasher.update(&hash.to_le_bytes());
    }
    for path in configuration_inputs(workspace) {
        update_path(&mut hasher, &path);
        hasher.update(&content_hash(&path).to_le_bytes());
    }
    hasher.digest()
}

fn configuration_inputs(workspace: &Workspace) -> BTreeSet<PathBuf> {
    let mut paths = BTreeSet::new();
    for package in &workspace.packages {
        paths.insert(package.root.join("package.json"));
        if let Some(tsconfig) = &package.tsconfig {
            paths.insert(tsconfig.clone());
        }
    }
    for name in [
        "unstave.toml",
        "pnpm-lock.yaml",
        "package-lock.json",
        "yarn.lock",
        "bun.lock",
        "bun.lockb",
    ] {
        paths.insert(workspace.root.join(name));
    }
    // Every tsconfig/jsconfig under the workspace root is a resolution input:
    // the resolver reads referenced configs too, so editing e.g.
    // `tsconfig.app.json` must invalidate the cached analysis even though that
    // file is not a source module and is not the package's own tsconfig.
    // These files are tiny, so walk for all of them.
    paths.extend(tsconfig_inputs(&workspace.root));
    paths
}

/// Every `tsconfig*.json` / `jsconfig*.json` file beneath `root`, in a
/// deterministic order. Build/dependency directories are skipped, mirroring
/// discovery's walk.
fn tsconfig_inputs(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|name| name.to_str());
                if !name.is_some_and(|name| ALWAYS_EXCLUDE_DIRS.contains(&name)) {
                    stack.push(path);
                }
            } else {
                let name = path.file_name().and_then(|name| name.to_str());
                if name.is_some_and(|name| {
                    (name.starts_with("tsconfig") || name.starts_with("jsconfig"))
                        && name.ends_with(".json")
                }) {
                    found.push(path);
                }
            }
        }
    }
    found.sort();
    found
}

fn content_hash(path: &Path) -> u64 {
    match std::fs::read(path) {
        Ok(bytes) => xxh3_64(&bytes),
        Err(error) => {
            let mut hasher = Xxh3::new();
            hasher.update(b"unreadable");
            hasher.update(&error.raw_os_error().unwrap_or_default().to_le_bytes());
            hasher.digest()
        }
    }
}

fn update_path(hasher: &mut Xxh3, path: &Path) {
    update_field(hasher, b"path", path.to_string_lossy().as_bytes());
}

fn update_field(hasher: &mut Xxh3, label: &[u8], value: &[u8]) {
    hasher.update(&(label.len() as u64).to_le_bytes());
    hasher.update(label);
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SCRATCH_ID: AtomicU64 = AtomicU64::new(0);

    fn scratch_fixture() -> PathBuf {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/simple");
        let target = std::env::temp_dir().join(format!(
            "unstave-cache-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos(),
            SCRATCH_ID.fetch_add(1, Ordering::Relaxed)
        ));
        copy_tree(&source, &target);
        target
    }

    fn scratch_fixture_named(name: &str) -> PathBuf {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(name);
        let target = std::env::temp_dir().join(format!(
            "unstave-cache-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos(),
            SCRATCH_ID.fetch_add(1, Ordering::Relaxed)
        ));
        copy_tree(&source, &target);
        target
    }

    fn copy_tree(source: &Path, target: &Path) {
        std::fs::create_dir_all(target).expect("create fixture directory");
        for entry in std::fs::read_dir(source).expect("read fixture") {
            let entry = entry.expect("fixture entry");
            if entry.file_name() == ".unstave" {
                continue;
            }
            let destination = target.join(entry.file_name());
            if entry.file_type().expect("file type").is_dir() {
                copy_tree(&entry.path(), &destination);
            } else {
                std::fs::copy(entry.path(), destination).expect("copy fixture file");
            }
        }
    }

    #[test]
    fn cache_hits_only_while_content_hashes_match() {
        let root = scratch_fixture();
        let config = Config::default();

        let cold = analyze_cached(&root, &config).expect("cold analysis");
        assert!(!cold.cache_hit);
        assert!(cache_path(&root).is_file());

        let warm = analyze_cached(&root, &config).expect("warm analysis");
        assert!(warm.cache_hit);
        assert_eq!(warm.modules.len(), cold.modules.len());

        let main = root.join("src/main.ts");
        let mut source = std::fs::read_to_string(&main).expect("read source");
        source.push_str("\nexport const cacheInvalidator = true;\n");
        std::fs::write(main, source).expect("change source");
        let changed = analyze_cached(&root, &config).expect("changed analysis");
        assert!(!changed.cache_hit);

        assert!(clear_cache(&root).expect("clear cache"));
        assert!(!clear_cache(&root).expect("clear missing cache"));
        std::fs::remove_dir_all(root).expect("remove scratch fixture");
    }

    #[test]
    fn corrupted_cache_is_a_safe_miss() {
        let root = scratch_fixture();
        let config = Config::default();
        analyze_cached(&root, &config).expect("populate cache");
        std::fs::write(cache_path(&root), b"not an archive").expect("corrupt cache");

        let result = analyze_cached(&root, &config).expect("recover from cache corruption");
        assert!(!result.cache_hit);
        std::fs::remove_dir_all(root).expect("remove scratch fixture");
    }

    #[test]
    fn editing_a_referenced_tsconfig_invalidates_the_cache() {
        // The split-tsconfig fixture's root `tsconfig.json` has no `paths`; the
        // `@/*` mapping lives in the referenced `tsconfig.app.json`. Editing that
        // referenced file changes resolution output, so the cache must miss even
        // though the package's own tsconfig path is unchanged.
        let root = scratch_fixture_named("split-tsconfig");
        let config = Config::default();

        let cold = analyze_cached(&root, &config).expect("cold analysis");
        assert!(!cold.cache_hit);

        let warm = analyze_cached(&root, &config).expect("warm analysis");
        assert!(warm.cache_hit);

        let app_tsconfig = root.join("tsconfig.app.json");
        let mut text = std::fs::read_to_string(&app_tsconfig).expect("read tsconfig.app.json");
        text = text.replace("\"@/*\": [\"src/*\"]", "\"@/*\": [\"./src/*\"]");
        std::fs::write(&app_tsconfig, text).expect("edit tsconfig.app.json");

        let changed = analyze_cached(&root, &config).expect("changed analysis");
        assert!(
            !changed.cache_hit,
            "editing a referenced tsconfig must invalidate the cached analysis"
        );

        std::fs::remove_dir_all(root).expect("remove scratch fixture");
    }
}
