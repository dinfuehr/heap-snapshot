mod common;
use common::{heap_snapshot_bin, test_dir};
use std::process::Command;

fn run_retainers(file: &str, object_id: &str, extra: &[&str]) -> String {
    let path = format!("{}/{}", test_dir(), file);
    let mut cmd = heap_snapshot_bin();
    cmd.arg("retainers").arg(&path).arg(object_id);
    for arg in extra {
        cmd.arg(arg);
    }
    let output = cmd.output().expect("failed to run heap-snapshot");
    assert!(output.status.success(), "exit code: {}", output.status);
    String::from_utf8(output.stdout).expect("invalid utf-8")
}

/// Find an InitialObject node ID from the summary output to use in retainers tests.
fn find_initial_object_id(file: &str) -> String {
    let path = format!("{}/{}", test_dir(), file);
    let output = Command::new(env!("CARGO_BIN_EXE_heap-snapshot"))
        .args(["summary", &path, "-g", "InitialObject"])
        .output()
        .expect("failed to run heap-snapshot");
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Find a line like "  ▶ InitialObject @21129" or "  ▶ InitialObject [file:1:2] @21129"
    let re = regex::Regex::new(r"InitialObject\b.*?@(\d+)").unwrap();
    for line in stdout.lines() {
        if let Some(caps) = re.captures(line) {
            return format!("@{}", &caps[1]);
        }
    }
    panic!("could not find an InitialObject node ID in summary output");
}

#[test]
fn retainers_shows_header() {
    let id = find_initial_object_id("heap-1.heapsnapshot");
    let output = run_retainers("heap-1.heapsnapshot", &id, &[]);
    assert!(
        output.contains(&format!("Retainers for InitialObject {id}:")),
        "expected retainers header for {id}"
    );
}

#[test]
fn retainers_shows_retainer_chain() {
    // @27945 is the `keep` InitialObject (largest retained size of the three
    // InitialObjects produced by snapshot_diffs.js).
    let output = run_retainers("heap-1.heapsnapshot", "@27945", &["--depth", "3"]);
    assert_content!(output, "expected_retainers_heap1_keep.txt");
}

#[test]
fn retainers_weakrefs_full_output() {
    // @7279 is the WeakTarget instance created in snapshot_weakrefs.js.
    let output = run_retainers("weakrefs.heapsnapshot", "@7279", &[]);
    assert_content!(output, "expected_retainers_weakrefs.txt");
}

#[test]
fn retainers_invalid_id_fails() {
    let path = format!("{}/heap-1.heapsnapshot", test_dir());
    let output = Command::new(env!("CARGO_BIN_EXE_heap-snapshot"))
        .args(["retainers", &path, "@999999999"])
        .output()
        .expect("failed to run heap-snapshot");
    assert!(
        !output.status.success(),
        "expected non-zero exit for invalid ID"
    );
}
