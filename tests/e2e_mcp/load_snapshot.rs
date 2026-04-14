use super::*;

#[test]
fn load_snapshot() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    let resp = proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));
    let text = get_text(&resp);
    let expected = format!(
        "Loaded snapshot from {}/heap-1.heapsnapshot with 10653 nodes. snapshot_id: 1",
        test_dir()
    );
    assert_eq!(text, expected);
}

#[test]
fn load_and_close_snapshot() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    let resp = proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));
    let text = get_text(&resp);
    let expected = format!(
        "Loaded snapshot from {}/heap-1.heapsnapshot with 10653 nodes. snapshot_id: 1",
        test_dir()
    );
    assert_eq!(text, expected);

    let resp = proc.call_tool(2, "close_snapshot", serde_json::json!({ "snapshot_id": 1 }));
    let text = get_text(&resp);
    assert_eq!(text, "Closed snapshot 1");
}

#[test]
fn close_nonexistent_snapshot() {
    let mut proc = McpProcess::start();

    let resp = proc.call_tool(
        1,
        "close_snapshot",
        serde_json::json!({ "snapshot_id": 999 }),
    );
    let text = get_text(&resp);
    assert_eq!(text, "No snapshot found with id 999");
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

    let text1 = get_text(&resp1);
    let text2 = get_text(&resp2);
    let expected1 = format!(
        "Loaded snapshot from {}/heap-1.heapsnapshot with 10653 nodes. snapshot_id: 1",
        test_dir()
    );
    let expected2 = format!(
        "Loaded snapshot from {}/heap-1.heapsnapshot with 10653 nodes. snapshot_id: 2",
        test_dir()
    );
    assert_eq!(text1, expected1);
    assert_eq!(text2, expected2);
}
