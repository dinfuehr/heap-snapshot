mod common;
use common::{heap_snapshot_bin, test_dir};

fn run_show(file: &str, object_id: &str, extra: &[&str]) -> String {
    let path = format!("{}/{}", test_dir(), file);
    let mut cmd = heap_snapshot_bin();
    cmd.arg("show").arg(&path).arg(object_id);
    for arg in extra {
        cmd.arg(arg);
    }
    let output = cmd.output().expect("failed to run heap-snapshot");
    assert!(output.status.success(), "exit code: {}", output.status);
    String::from_utf8(output.stdout).expect("invalid utf-8")
}

#[test]
fn show_prints_object_header() {
    let output = run_show("heap-1.heapsnapshot", "@3", &[]);
    assert!(
        output.contains("id:           @3"),
        "expected id line, got: {output}"
    );
    assert!(
        output.contains("name:         (GC roots)"),
        "expected (GC roots) name, got: {output}"
    );
}

#[test]
fn show_prints_outgoing_edges() {
    let output = run_show("heap-1.heapsnapshot", "@3", &[]);
    assert!(
        output.contains("--["),
        "expected outgoing edges, got: {output}"
    );
}

#[test]
fn show_depth_1_has_no_nested_edges() {
    let output = run_show("heap-1.heapsnapshot", "@3", &[]);
    let nested = output.lines().filter(|l| l.starts_with("    --[")).count();
    assert_eq!(nested, 0, "depth=1 should not have nested edges");
}

#[test]
fn show_depth_2_has_nested_edges() {
    let output = run_show("heap-1.heapsnapshot", "@3", &["--depth", "2"]);
    let nested = output.lines().filter(|l| l.starts_with("    --[")).count();
    assert!(
        nested > 0,
        "depth=2 should have nested edges, got: {output}"
    );
}

#[test]
fn show_limit_restricts_children() {
    let output = run_show("heap-1.heapsnapshot", "@3", &["--limit", "3"]);
    let edges: Vec<_> = output.lines().filter(|l| l.starts_with("  --[")).collect();
    assert!(
        edges.len() <= 3,
        "expected at most 3 edges with --limit 3, got: {}",
        edges.len()
    );
    assert!(
        output.contains("children shown"),
        "expected truncation message, got: {output}"
    );
}

#[test]
fn show_offset_skips_first() {
    let all = run_show("heap-1.heapsnapshot", "@3", &[]);
    let offset = run_show("heap-1.heapsnapshot", "@3", &["--offset", "1"]);

    let all_edges: Vec<_> = all.lines().filter(|l| l.starts_with("  --[")).collect();
    let offset_edges: Vec<_> = offset.lines().filter(|l| l.starts_with("  --[")).collect();

    assert!(
        offset_edges.len() < all_edges.len(),
        "offset should reduce edges shown"
    );
    if all_edges.len() > 1 {
        assert_eq!(
            offset_edges[0], all_edges[1],
            "offset=1 should skip the first edge"
        );
    }
}

#[test]
fn show_without_at_prefix() {
    let output = run_show("heap-1.heapsnapshot", "3", &[]);
    assert!(
        output.contains("id:           @3"),
        "object_id without @ prefix should work, got: {output}"
    );
}

#[test]
fn show_exact_output() {
    let output = run_show(
        "heap-1.heapsnapshot",
        "@7165",
        &["--depth", "0", "--limit", "3"],
    );
    let expected = "\
id:           @7165
ordinal:      3582
type:         native
name:         system / NativeContext
class:        system / NativeContext
self size:    1 kB (1240)
retained:     23 kB (23708)
distance:     2
detachedness: Unknown
edge count:   265
  --[internal \"scope_info\"]--> @413 system / ScopeInfo (type: code, self_size: 0)
  --[internal \"global_object\"]--> @7531 global [JSGlobalObject] (type: object, self_size: 24)
  --[internal \"global_proxy_object\"]--> @7181 global [JSGlobalProxy] (type: object, self_size: 16)
  (1-3 of 265 children shown)
";
    assert_eq!(output, expected);
}
