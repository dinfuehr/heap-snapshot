use super::*;

#[test]
fn get_reachable_size() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_reachable_size",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1" }),
    );
    assert_content!(get_text(&resp), "expected_mcp_get_reachable_size_root.txt");
}

#[test]
fn get_reachable_size_reaches_native_context() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    // @25 is the stable (Handle scope) id — reaches the utility native context.
    let resp = proc.call_tool(
        2,
        "get_reachable_size",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@25" }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_get_reachable_size_handle_scope.txt"
    );
}

#[test]
fn get_reachable_size_invalid_object() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_reachable_size",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@999999999" }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("No object found"),
        "expected not-found error, got: {err}"
    );
}
