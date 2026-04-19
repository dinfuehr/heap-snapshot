use super::*;

#[test]
fn load_snapshot() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    let resp = proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));
    assert_content!(get_text(&resp), "expected_mcp_load_snapshot_1.txt");
}

#[test]
fn load_and_close_snapshot() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    let resp = proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));
    assert_content!(get_text(&resp), "expected_mcp_load_snapshot_1.txt");

    let resp = proc.call_tool(2, "close_snapshot", serde_json::json!({ "snapshot_id": 1 }));
    assert_content!(get_text(&resp), "expected_mcp_close_snapshot.txt");
}

#[test]
fn close_nonexistent_snapshot() {
    let mut proc = McpProcess::start();

    let resp = proc.call_tool(
        1,
        "close_snapshot",
        serde_json::json!({ "snapshot_id": 999 }),
    );
    assert_content!(get_text(&resp), "expected_mcp_close_snapshot_missing.txt");
}

#[test]
fn load_nonexistent_file() {
    let mut proc = McpProcess::start();

    let resp = proc.call_tool(
        1,
        "load_snapshot",
        serde_json::json!({ "path": "/nonexistent/file.heapsnapshot" }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("Failed to open"),
        "expected open error, got: {err}"
    );
}

#[test]
fn multiple_snapshots_get_different_ids() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    let resp1 = proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": &path }));
    let resp2 = proc.call_tool(2, "load_snapshot", serde_json::json!({ "path": &path }));

    assert_content!(get_text(&resp1), "expected_mcp_load_snapshot_1.txt");
    assert_content!(get_text(&resp2), "expected_mcp_load_snapshot_2.txt");
}
