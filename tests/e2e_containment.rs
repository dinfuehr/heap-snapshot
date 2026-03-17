mod common;
use common::{heap_snapshot_bin, test_dir};
use std::process::Command;

fn run_containment(file: &str, extra: &[&str]) -> String {
    let path = format!("{}/{}", test_dir(), file);
    let mut cmd = heap_snapshot_bin();
    cmd.arg("containment").arg(&path);
    for arg in extra {
        cmd.arg(arg);
    }
    let output = cmd.output().expect("failed to run heap-snapshot");
    assert!(output.status.success(), "exit code: {}", output.status);
    String::from_utf8(output.stdout).expect("invalid utf-8")
}

#[test]
fn containment_shows_gc_roots() {
    let output = run_containment("heap-1.heapsnapshot", &["--depth", "1"]);
    assert!(
        output.contains("(GC roots)"),
        "expected (GC roots) in containment tree"
    );
}

#[test]
fn containment_shows_header() {
    let output = run_containment("heap-1.heapsnapshot", &["--depth", "1"]);
    assert!(
        output.contains("Containment for"),
        "expected containment header"
    );
}

#[test]
fn containment_depth_0_shows_only_root() {
    let output = run_containment("heap-1.heapsnapshot", &[]);
    // depth=0 (default): only root edges shown, no nested expansion markers
    // Root itself + its direct children, but children are collapsed (▶)
    let expanded_count = output
        .lines()
        .filter(|l| l.contains('\u{25bc}')) // ▼
        .count();
    // Only the root's direct children listing, no deep expansion
    // With depth=0, nothing is auto-expanded beyond the root
    assert!(
        expanded_count <= 1,
        "expected at most 1 expanded node at depth=0, got {expanded_count}"
    );
}

#[test]
fn containment_depth_2_shows_nested_edges() {
    let output = run_containment("heap-1.heapsnapshot", &["--depth", "2"]);
    // With depth=2, we should see edge pagination ("of ... refs")
    assert!(
        output.contains("refs"),
        "expected edge refs count at depth=2"
    );
}

#[test]
fn containment_specific_node() {
    // Find an InitialObject ID from summary, then show its containment
    let summary_path = format!("{}/heap-1.heapsnapshot", test_dir());
    let summary_out = Command::new(env!("CARGO_BIN_EXE_heap-snapshot"))
        .args(["summary", &summary_path, "-g", "InitialObject"])
        .output()
        .expect("failed to run summary");
    let stdout = String::from_utf8(summary_out.stdout).unwrap();

    let mut node_id = String::new();
    for line in stdout.lines() {
        if let Some(pos) = line.find("InitialObject @") {
            let after_at = &line[pos + "InitialObject @".len()..];
            node_id = format!(
                "@{}",
                after_at
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
            );
            break;
        }
    }
    assert!(!node_id.is_empty(), "could not find InitialObject node ID");

    let output = run_containment("heap-1.heapsnapshot", &[&node_id, "--depth", "1"]);
    assert!(
        output.contains(&format!("InitialObject {node_id}")),
        "expected containment header mentioning {node_id}"
    );
}
