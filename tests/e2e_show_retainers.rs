mod common;
use common::{heap_snapshot_bin, test_dir};

fn run_show_retainers(file: &str, object_id: &str, extra: &[&str]) -> String {
    let path = format!("{}/{}", test_dir(), file);
    let mut cmd = heap_snapshot_bin();
    cmd.arg("show-retainers").arg(&path).arg(object_id);
    for arg in extra {
        cmd.arg(arg);
    }
    let output = cmd.output().expect("failed to run heap-snapshot");
    assert!(output.status.success(), "exit code: {}", output.status);
    String::from_utf8(output.stdout).expect("invalid utf-8")
}

#[test]
fn show_retainers_prints_object_header() {
    let output = run_show_retainers("heap-1.heapsnapshot", "@7165", &[]);
    assert!(
        output.contains("Object @7165:"),
        "expected object header, got: {output}"
    );
    assert!(
        output.contains("retained_size:"),
        "expected retained_size in header, got: {output}"
    );
}

#[test]
fn show_retainers_prints_incoming_edges() {
    let output = run_show_retainers("heap-1.heapsnapshot", "@7165", &[]);
    assert!(
        output.contains("<--["),
        "expected incoming edges, got: {output}"
    );
}

#[test]
fn show_retainers_depth_1_has_no_nested() {
    let output = run_show_retainers("heap-1.heapsnapshot", "@7165", &[]);
    let nested = output.lines().filter(|l| l.starts_with("    <--[")).count();
    assert_eq!(nested, 0, "depth=1 should not have nested retainers");
}

#[test]
fn show_retainers_depth_2_has_nested() {
    let output = run_show_retainers("heap-1.heapsnapshot", "@7165", &["--depth", "2"]);
    let nested = output.lines().filter(|l| l.starts_with("    <--[")).count();
    assert!(
        nested > 0,
        "depth=2 should have nested retainers, got: {output}"
    );
}

#[test]
fn show_retainers_limit_restricts() {
    let output = run_show_retainers("heap-1.heapsnapshot", "@7165", &["--limit", "2"]);
    let edges: Vec<_> = output.lines().filter(|l| l.starts_with("  <--[")).collect();
    assert!(
        edges.len() <= 2,
        "expected at most 2 retainers with --limit 2, got: {}",
        edges.len()
    );
}

#[test]
fn show_retainers_without_at_prefix() {
    let output = run_show_retainers("heap-1.heapsnapshot", "7165", &[]);
    assert!(
        output.contains("Object @7165:"),
        "object_id without @ prefix should work, got: {output}"
    );
}
