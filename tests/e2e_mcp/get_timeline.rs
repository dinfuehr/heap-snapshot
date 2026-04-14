use super::*;

#[test]
fn get_timeline_no_data() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(2, "get_timeline", serde_json::json!({ "snapshot_id": 1 }));
    let text = get_text(&resp);
    assert_eq!(text, "No allocation timeline data in this snapshot.");
}

#[test]
fn get_timeline_invalid_snapshot() {
    let mut proc = McpProcess::start();

    let resp = proc.call_tool(1, "get_timeline", serde_json::json!({ "snapshot_id": 999 }));
    let err = get_error_message(&resp);
    assert!(
        err.contains("No snapshot found"),
        "expected not-found error, got: {err}"
    );
}
