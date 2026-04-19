use super::*;

#[test]
fn get_dominators_of() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    // @25 is the stable (Handle scope) node — ultimately dominated by @3 (GC roots).
    let resp = proc.call_tool(
        2,
        "get_dominators_of",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@25" }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_get_dominators_of_handle_scope.txt"
    );
}
