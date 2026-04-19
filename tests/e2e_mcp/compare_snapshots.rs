use super::*;

fn load(proc: &mut McpProcess, id: u64, file: &str) {
    let path = format!("{}/{}", test_dir(), file);
    proc.call_tool(id, "load_snapshot", serde_json::json!({ "path": path }));
}

#[test]
fn compare_snapshots_heap2_vs_heap1_new_objects() {
    let mut proc = McpProcess::start();
    load(&mut proc, 1, "heap-1.heapsnapshot");
    load(&mut proc, 2, "heap-2.heapsnapshot");

    let resp = proc.call_tool(
        3,
        "compare_snapshots",
        serde_json::json!({ "snapshot_id": 2, "baseline_id": 1 }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_compare_snapshots_heap2_vs_heap1.txt"
    );
}

#[test]
fn compare_snapshots_heap2_vs_heap1_deleted_objects() {
    let mut proc = McpProcess::start();
    load(&mut proc, 1, "heap-1.heapsnapshot");
    load(&mut proc, 2, "heap-2.heapsnapshot");

    let resp = proc.call_tool(
        3,
        "compare_snapshots",
        serde_json::json!({ "snapshot_id": 2, "baseline_id": 1 }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_compare_snapshots_heap2_vs_heap1.txt"
    );
}

#[test]
fn compare_snapshots_heap3_vs_heap1() {
    let mut proc = McpProcess::start();
    load(&mut proc, 1, "heap-1.heapsnapshot");
    load(&mut proc, 2, "heap-3.heapsnapshot");

    let resp = proc.call_tool(
        3,
        "compare_snapshots",
        serde_json::json!({ "snapshot_id": 2, "baseline_id": 1 }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_compare_snapshots_heap3_vs_heap1.txt"
    );
}

#[test]
fn compare_snapshots_identical() {
    let mut proc = McpProcess::start();
    load(&mut proc, 1, "heap-1.heapsnapshot");
    load(&mut proc, 2, "heap-1.heapsnapshot");

    let resp = proc.call_tool(
        3,
        "compare_snapshots",
        serde_json::json!({ "snapshot_id": 1, "baseline_id": 2 }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_compare_snapshots_identical.txt"
    );
}

#[test]
fn compare_snapshots_expand_class() {
    let mut proc = McpProcess::start();
    load(&mut proc, 1, "heap-1.heapsnapshot");
    load(&mut proc, 2, "heap-2.heapsnapshot");

    let resp = proc.call_tool(
        3,
        "compare_snapshots",
        serde_json::json!({
            "snapshot_id": 2,
            "baseline_id": 1,
            "class_name": "NewObject"
        }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_compare_snapshots_heap2_vs_heap1_newobject.txt"
    );
}

#[test]
fn compare_snapshots_expand_class_with_limit() {
    let mut proc = McpProcess::start();
    load(&mut proc, 1, "heap-1.heapsnapshot");
    load(&mut proc, 2, "heap-3.heapsnapshot");

    let resp = proc.call_tool(
        3,
        "compare_snapshots",
        serde_json::json!({
            "snapshot_id": 2,
            "baseline_id": 1,
            "class_name": "NewObject",
            "limit": 3
        }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_compare_snapshots_heap3_vs_heap1_newobject_limit3.txt"
    );
}

#[test]
fn compare_snapshots_reversed() {
    let mut proc = McpProcess::start();
    load(&mut proc, 1, "heap-1.heapsnapshot");
    load(&mut proc, 2, "heap-2.heapsnapshot");

    let resp = proc.call_tool(
        3,
        "compare_snapshots",
        serde_json::json!({ "snapshot_id": 1, "baseline_id": 2 }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_compare_snapshots_heap1_vs_heap2.txt"
    );
}

#[test]
fn compare_snapshots_invalid_class_name() {
    let mut proc = McpProcess::start();
    load(&mut proc, 1, "heap-1.heapsnapshot");
    load(&mut proc, 2, "heap-2.heapsnapshot");

    let resp = proc.call_tool(
        3,
        "compare_snapshots",
        serde_json::json!({
            "snapshot_id": 2,
            "baseline_id": 1,
            "class_name": "NoSuchConstructor"
        }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("No diff entry for constructor"),
        "expected not-found error, got: {err}"
    );
}

#[test]
fn compare_snapshots_invalid_snapshot_id() {
    let mut proc = McpProcess::start();
    load(&mut proc, 1, "heap-1.heapsnapshot");

    let resp = proc.call_tool(
        2,
        "compare_snapshots",
        serde_json::json!({ "snapshot_id": 1, "baseline_id": 99 }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("No snapshot found with id 99"),
        "expected missing snapshot error, got: {err}"
    );
}
