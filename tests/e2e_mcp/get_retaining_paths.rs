use super::*;

#[test]
fn get_retaining_paths_returns_success_on_limits() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    // Use a tiny depth budget to trigger the "no path within current limits"
    // note without turning the result into an MCP error.
    let resp = proc.call_tool(
        2,
        "get_retaining_paths",
        serde_json::json!({
            "snapshot_id": 1,
            "object_id": "@7165",
            "max_depth": 1,
            "max_nodes": 1
        }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_get_retaining_paths_limited_success.txt"
    );
}

#[test]
fn get_retaining_paths_returns_success_when_truncated() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_retaining_paths",
        serde_json::json!({
            "snapshot_id": 1,
            "object_id": "@7165",
            "max_depth": 50,
            "max_nodes": 1
        }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_get_retaining_paths_truncated_success.txt"
    );
}

#[test]
fn get_retaining_paths_root_target_omits_no_path_note() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_retaining_paths",
        serde_json::json!({
            "snapshot_id": 1,
            "object_id": "@3",
            "max_depth": 0,
            "max_nodes": 0
        }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_get_retaining_paths_root_target.txt"
    );
}
