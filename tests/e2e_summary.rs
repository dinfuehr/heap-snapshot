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
    // heap-1 has 3 InitialObjects
    assert!(
        output.contains("InitialObject  \u{00d7}3"),
        "expected InitialObject ×3"
    );
}

#[test]
fn summary_lists_new_objects_in_heap2() {
    let output = run_summary("heap-2.heapsnapshot", &[]);
    assert!(
        output.contains("NewObject"),
        "expected NewObject in heap-2 summary"
    );
    assert!(
        output.contains("NewObject  \u{00d7}2"),
        "expected NewObject ×2"
    );
}

#[test]
fn summary_lists_new_objects_in_heap3() {
    let output = run_summary("heap-3.heapsnapshot", &[]);
    assert!(
        output.contains("NewObject  \u{00d7}7"),
        "expected NewObject ×7 in heap-3"
    );
}

#[test]
fn summary_expand_group_shows_members() {
    let output = run_summary("heap-1.heapsnapshot", &["-g", "InitialObject"]);
    // Expanded: ▼ marker on the group
    assert!(
        output.contains("\u{25bc} InitialObject"),
        "expected ▼ marker on expanded group"
    );
    // Individual members: "▶ InitialObject @"
    let member_count = output
        .lines()
        .filter(|l| l.contains("\u{25b6} InitialObject @"))
        .count();
    assert_eq!(member_count, 3, "expected 3 expanded InitialObject members");
}

#[test]
fn summary_expand_group_with_window() {
    let output = run_summary("heap-1.heapsnapshot", &["-g", "InitialObject:0:2"]);
    let member_count = output
        .lines()
        .filter(|l| l.contains("\u{25b6} InitialObject @"))
        .count();
    assert_eq!(member_count, 2, "expected 2 members with window :0:2");
    assert!(
        output.contains("of 3 members"),
        "expected 'of 3 members' status line"
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
    assert!(stdout.contains("Statistics"), "expected Statistics section");
    assert!(stdout.contains("V8 Heap:"), "expected V8 Heap stat");
}
