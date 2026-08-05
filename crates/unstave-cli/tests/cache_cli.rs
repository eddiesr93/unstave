use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SCRATCH_ID: AtomicU64 = AtomicU64::new(0);

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/simple")
}

fn scratch_fixture() -> PathBuf {
    let target = std::env::temp_dir().join(format!(
        "unstave-cache-cli-test-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos(),
        SCRATCH_ID.fetch_add(1, Ordering::Relaxed)
    ));
    copy_tree(&fixture(), &target);
    target
}

fn copy_tree(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).expect("create scratch fixture");
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
fn default_analysis_caches_then_cache_clear_removes_it() {
    let root = scratch_fixture();
    let root_arg = root.to_str().expect("utf-8 path");

    let cold = Command::new(env!("CARGO_BIN_EXE_unstave"))
        .args(["analyze", "--root", root_arg, "--verbose"])
        .output()
        .expect("run cold analysis");
    assert!(cold.status.success());
    assert!(String::from_utf8_lossy(&cold.stderr).contains("cache: miss"));
    assert!(root.join(".unstave/cache-v1.rkyv").is_file());

    let warm = Command::new(env!("CARGO_BIN_EXE_unstave"))
        .args(["analyze", "--root", root_arg, "--verbose"])
        .output()
        .expect("run warm analysis");
    assert!(warm.status.success());
    assert!(String::from_utf8_lossy(&warm.stderr).contains("cache: hit"));

    let clear = Command::new(env!("CARGO_BIN_EXE_unstave"))
        .args(["cache", "clear", "--root", root_arg])
        .output()
        .expect("clear cache");
    assert!(clear.status.success());
    assert!(String::from_utf8_lossy(&clear.stdout).contains("cleared"));
    assert!(!root.join(".unstave/cache-v1.rkyv").exists());
    std::fs::remove_dir_all(root).expect("remove scratch fixture");
}

#[test]
fn no_cache_bypasses_reads_and_writes() {
    let root = scratch_fixture();
    let output = Command::new(env!("CARGO_BIN_EXE_unstave"))
        .args([
            "analyze",
            "--root",
            root.to_str().expect("utf-8 path"),
            "--no-cache",
            "--verbose",
        ])
        .output()
        .expect("run uncached analysis");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cache: disabled"));
    assert!(!root.join(".unstave/cache-v1.rkyv").exists());
    std::fs::remove_dir_all(root).expect("remove scratch fixture");
}
