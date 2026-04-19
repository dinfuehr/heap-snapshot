use super::*;

#[test]
fn get_native_contexts() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_native_contexts",
        serde_json::json!({ "snapshot_id": 1 }),
    );
    assert_content!(get_text(&resp), "expected_mcp_get_native_contexts.txt");
}

#[test]
fn get_native_contexts_invalid_snapshot() {
    let mut proc = McpProcess::start();

    let resp = proc.call_tool(
        1,
        "get_native_contexts",
        serde_json::json!({ "snapshot_id": 999 }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("No snapshot found"),
        "expected not-found error, got: {err}"
    );
}
