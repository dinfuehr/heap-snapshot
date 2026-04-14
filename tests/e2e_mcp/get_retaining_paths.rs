use super::*;

#[test]
fn get_retaining_paths_error_on_limits() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    // Use very small limits to trigger truncation
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
    let err = get_error_message(&resp);
    assert!(
        err.contains("Retaining paths for @7165"),
        "expected retaining paths header in error, got: {err}"
    );
}
