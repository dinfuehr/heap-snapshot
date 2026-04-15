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
    let text = get_text(&resp);
    assert_eq!(
        text,
        "1 native contexts:\n@7165 [utility] #0 @7165 (detachedness: unknown, self_size: 1240, retained_size: 23708)"
    );
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
