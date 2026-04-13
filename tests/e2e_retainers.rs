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
    let id = find_initial_object_id("heap-1.heapsnapshot");
    let output = run_retainers("heap-1.heapsnapshot", &id, &["--depth", "3"]);
    // The InitialObject is retained via "a in {a, b, keep}" or similar
    // and eventually reaches a GC root holder (e.g. Handle scope, Stack roots).
    let reaches_root_holder = output.contains("(Handle scope)")
        || output.contains("(Stack roots)")
        || output.contains("(Global handles)")
        || output.contains("(Strong root list)");
    assert!(
        reaches_root_holder,
        "expected retainer chain to reach a GC root holder"
    );
}

#[test]
fn retainers_weakrefs_7207_full_output() {
    let output = run_retainers("weakrefs.heapsnapshot", "@7207", &[]);
    let expected = include_str!("data/expected_retainers_7207.txt");

    // Strip status lines ("Reading …", "Initializing …", blank line) from
    // the beginning of the actual output, then compare line-by-line with
    // trailing whitespace stripped.
    let actual_lines: Vec<&str> = output
        .lines()
        .skip_while(|l| !l.starts_with("Retainers for"))
        .map(|l| l.trim_end())
        .collect();
    let expected_lines: Vec<&str> = expected.lines().map(|l| l.trim_end()).collect();

    if actual_lines != expected_lines {
        // Build a readable diff.
        let max = actual_lines.len().max(expected_lines.len());
        let mut diffs = Vec::new();
        for i in 0..max {
            let a = actual_lines.get(i).copied().unwrap_or("<missing>");
            let e = expected_lines.get(i).copied().unwrap_or("<missing>");
            if a != e {
                diffs.push(format!("line {i}:\n  expected: {e:?}\n  actual:   {a:?}"));
            }
        }
        panic!(
            "retainers output mismatch ({} lines differ, {} actual vs {} expected):\n{}",
            diffs.len(),
            actual_lines.len(),
            expected_lines.len(),
            diffs.join("\n"),
        );
    }
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
