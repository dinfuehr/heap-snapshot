use super::*;

#[test]
fn get_closure_leaks_no_leaks() {
    let mut proc = McpProcess::start();
    let path = format!("{}/closures.heapsnapshot", test_dir());
    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "get_closure_leaks",
        serde_json::json!({ "snapshot_id": 1 }),
    );
    let text = get_text(&resp);
    assert_eq!(text, "No closure leaks detected.");
}

#[test]
fn get_closure_leaks_show_incomplete() {
    let mut proc = McpProcess::start();
    let path = format!("{}/closures.heapsnapshot", test_dir());
    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "get_closure_leaks",
        serde_json::json!({ "snapshot_id": 1, "show_incomplete": true }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_get_closure_leaks_show_incomplete.txt"
    );
}

#[test]
fn get_closure_leaks_invalid_snapshot() {
    let mut proc = McpProcess::start();
    let resp = proc.call_tool(
        1,
        "get_closure_leaks",
        serde_json::json!({ "snapshot_id": 999 }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("No snapshot found"),
        "expected not-found error, got: {err}"
    );
}
