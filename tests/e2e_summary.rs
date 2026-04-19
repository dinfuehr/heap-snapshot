mod common;
use common::{heap_snapshot_bin, test_dir};

fn run_summary(file: &str, extra: &[&str]) -> String {
    let path = format!("{}/{}", test_dir(), file);
    let mut cmd = heap_snapshot_bin();
    cmd.arg("summary").arg(&path);
    for arg in extra {
        cmd.arg(arg);
    }
    let output = cmd.output().expect("failed to run heap-snapshot");
    assert!(output.status.success(), "exit code: {}", output.status);
    String::from_utf8(output.stdout).expect("invalid utf-8")
}

#[test]
fn summary_lists_initial_objects() {
    let output = run_summary("heap-1.heapsnapshot", &[]);
    assert!(
        output.contains("InitialObject"),
        "expected InitialObject in summary"
    );
    // heap-1 has 4 InitialObjects
    assert!(
        output.contains("\u{00d7}4"),
        "expected ×4 for InitialObject"
    );
}

#[test]
fn summary_lists_new_objects_in_heap2() {
    let output = run_summary("heap-2.heapsnapshot", &[]);
    assert!(
        output.contains("NewObject"),
        "expected NewObject in heap-2 summary"
    );
    let re = regex::Regex::new(r"NewObject\b.*?\u{00d7}2").unwrap();
    assert!(re.is_match(&output), "expected NewObject ×2");
}

#[test]
fn summary_lists_new_objects_in_heap3() {
    let output = run_summary("heap-3.heapsnapshot", &[]);
    let re = regex::Regex::new(r"NewObject\b.*?\u{00d7}7").unwrap();
    assert!(re.is_match(&output), "expected NewObject ×7 in heap-3");
}

#[test]
fn summary_expand_group_shows_members() {
    let output = run_summary("heap-1.heapsnapshot", &["-g", "InitialObject"]);
    // Expanded: ▼ marker on the group
    assert!(
        output.contains("\u{25bc} InitialObject"),
        "expected ▼ marker on expanded group"
    );
    // Individual members: "▶ InitialObject ... @<id>"
    let re = regex::Regex::new(r"\u{25b6} InitialObject\b.*@\d+").unwrap();
    let member_count = output.lines().filter(|l| re.is_match(l)).count();
    // snapshot_diffs.js creates 3 InitialObjects (a, b, keep).
    assert_eq!(member_count, 3, "expected 3 expanded InitialObject members");
}

#[test]
fn summary_expand_group_with_window() {
    let output = run_summary("heap-1.heapsnapshot", &["-g", "InitialObject:0:2"]);
    let re = regex::Regex::new(r"\u{25b6} InitialObject\b.*@\d+").unwrap();
    let member_count = output.lines().filter(|l| re.is_match(l)).count();
    assert_eq!(member_count, 2, "expected 2 members with window :0:2");
    assert!(
        output.contains("of 3 members"),
        "expected 'of 3 members' status line, got: {output}"
    );
}

#[test]
fn summary_shows_totals_line() {
    let output = run_summary("heap-1.heapsnapshot", &[]);
    assert!(
        output.contains("Total ("),
        "expected totals line in summary"
    );
}

#[test]
fn summary_shows_statistics() {
    let path = format!("{}/{}", test_dir(), "heap-1.heapsnapshot");
    let mut cmd = heap_snapshot_bin();
    cmd.arg("statistics").arg(&path);
    let output = cmd.output().expect("failed to run heap-snapshot");
    assert!(output.status.success(), "exit code: {}", output.status);
    let stdout = String::from_utf8(output.stdout).expect("invalid utf-8");
    // Skip progress lines (Reading/Initializing)
    let stats_output: String = stdout
        .lines()
        .filter(|l| !l.starts_with("Reading") && !l.starts_with("Initializing"))
        .collect::<Vec<_>>()
        .join("\n");
    let expected = "\
Statistics (total 125 kB):
  V8 Heap:        105 kB
    Code:         8 kB
    Strings:      6 kB
    JS Arrays:    64 B
    System:       0 B
  Native:         20 kB
    Typed Arrays: 0 B
    Extra Native: 0 B
  Unreachable:    0 B (0 objects)

Native Context Attribution:
  [utility] #0 @7271                       118 kB
  Shared                                   0 B
  Unattributed                             7 kB";
    assert_eq!(stats_output, expected);
}

#[test]
fn summary_filter_unreachable() {
    let output = run_summary("heap-1.heapsnapshot", &["--filter", "unreachable"]);
    // heap-1 has 0 unreachable objects
    assert!(
        output.contains("No matching objects found"),
        "expected no matching objects for unreachable filter, got: {output}"
    );
}

#[test]
fn summary_filter_unreachable_roots() {
    let output = run_summary("heap-1.heapsnapshot", &["--filter", "unreachable-roots"]);
    assert!(
        output.contains("No matching objects found"),
        "expected no matching objects for unreachable-roots filter, got: {output}"
    );
}

#[test]
fn summary_filter_detached_dom() {
    let output = run_summary("heap-1.heapsnapshot", &["--filter", "detached-dom"]);
    // Should not crash; may or may not have detached DOM nodes
    assert!(
        output.contains("Computing aggregates") || output.contains("No matching objects"),
        "expected valid output for detached-dom filter, got: {output}"
    );
}

#[test]
fn summary_filter_console() {
    let output = run_summary("heap-1.heapsnapshot", &["--filter", "console"]);
    assert!(
        output.contains("Computing aggregates") || output.contains("No matching objects"),
        "expected valid output for console filter, got: {output}"
    );
}

#[test]
fn summary_filter_event_handlers() {
    let output = run_summary("heap-1.heapsnapshot", &["--filter", "event-handlers"]);
    assert!(
        output.contains("Computing aggregates") || output.contains("No matching objects"),
        "expected valid output for event-handlers filter, got: {output}"
    );
}

#[test]
fn summary_filter_invalid() {
    let path = format!("{}/{}", test_dir(), "heap-1.heapsnapshot");
    let mut cmd = heap_snapshot_bin();
    cmd.arg("summary").arg(&path).arg("--filter").arg("bogus");
    let output = cmd.output().expect("failed to run heap-snapshot");
    assert!(
        !output.status.success(),
        "expected non-zero exit for invalid filter"
    );
    let stderr = String::from_utf8(output.stderr).expect("invalid utf-8");
    assert!(
        stderr.contains("unknown filter"),
        "expected error message about unknown filter, got: {stderr}"
    );
}
