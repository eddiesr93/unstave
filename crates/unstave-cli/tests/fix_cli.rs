use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn scratch_fixture(name: &str) -> PathBuf {
    let unique = format!(
        "unstave-fix-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos()
    );
    let target = std::env::temp_dir().join(unique);
    copy_tree(&fixture(name), &target);
    target
}

fn copy_tree(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).expect("create scratch fixture");
    for entry in std::fs::read_dir(source).expect("read fixture") {
        let entry = entry.expect("fixture entry");
        let destination = target.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            std::fs::copy(entry.path(), destination).expect("copy fixture file");
        }
    }
}

#[test]
fn dry_run_prints_a_diff_without_modifying_the_file() {
    let root = scratch_fixture("pure-barrel");
    let main = root.join("src/main.ts");
    let before = std::fs::read_to_string(&main).expect("read original");

    let output = Command::new(env!("CARGO_BIN_EXE_unstave"))
        .args([
            "fix",
            "--root",
            root.to_str().expect("utf-8 path"),
            "--import-style",
            "relative",
        ])
        .output()
        .expect("run unstave");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    assert!(stdout.contains("--- a/src/main.ts"));
    assert!(stdout.contains("+import { AlphaClient } from './clients/alpha';"));
    assert_eq!(
        std::fs::read_to_string(main).expect("read after dry-run"),
        before
    );
}

#[test]
fn check_exits_one_when_rewrites_are_needed_without_modifying_files() {
    let root = scratch_fixture("pure-barrel");
    let main = root.join("src/main.ts");
    let before = std::fs::read_to_string(&main).expect("read original");

    let output = Command::new(env!("CARGO_BIN_EXE_unstave"))
        .args([
            "fix",
            "--root",
            root.to_str().expect("utf-8 path"),
            "--check",
            "--import-style",
            "relative",
        ])
        .output()
        .expect("run unstave");

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("1 file(s) would change"));
    assert_eq!(
        std::fs::read_to_string(main).expect("read after check"),
        before
    );
}

#[test]
fn write_applies_the_plan_reports_a_summary_and_is_idempotent() {
    let root = scratch_fixture("pure-barrel");
    let root_arg = root.to_str().expect("utf-8 path");

    let output = Command::new(env!("CARGO_BIN_EXE_unstave"))
        .args([
            "fix",
            "--root",
            root_arg,
            "--write",
            "--import-style",
            "relative",
        ])
        .output()
        .expect("run unstave");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout)
        .contains("1 file(s) changed, 1 import(s) rewritten"));
    let rewritten = std::fs::read_to_string(root.join("src/main.ts")).expect("read rewritten");
    assert!(rewritten.starts_with("import { AlphaClient } from './clients/alpha';"));

    let check = Command::new(env!("CARGO_BIN_EXE_unstave"))
        .args(["fix", "--root", root_arg, "--check"])
        .output()
        .expect("run unstave check");
    assert!(
        check.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&check.stderr)
    );
}

#[test]
fn write_summary_groups_skipped_imports_by_reason() {
    let root = scratch_fixture("side-effects");
    let output = Command::new(env!("CARGO_BIN_EXE_unstave"))
        .args([
            "fix",
            "--root",
            root.to_str().expect("utf-8 path"),
            "--write",
        ])
        .output()
        .expect("run unstave");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("0 file(s) changed, 0 import(s) rewritten"));
    assert!(stdout.contains("barrel has side effects: 1"));
}
