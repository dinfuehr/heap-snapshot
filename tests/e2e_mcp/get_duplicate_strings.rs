use super::*;

#[test]
fn get_duplicate_strings_basic() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_duplicate_strings",
        serde_json::json!({ "snapshot_id": 1 }),
    );
    let text = get_text(&resp);
    // heap-1.heapsnapshot lacks length edges, so all strings are skipped.
    assert!(
        text.contains("0 duplicate string groups"),
        "expected 0 groups (no length metadata), got: {text}"
    );
    assert!(
        text.contains("skipped"),
        "should report skipped strings, got: {text}"
    );
}

#[test]
fn get_duplicate_strings_pagination() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_duplicate_strings",
        serde_json::json!({ "snapshot_id": 1, "limit": 2 }),
    );
    let text = get_text(&resp);
    assert!(
        text.contains("0 duplicate string groups"),
        "expected 0 groups, got: {text}"
    );
}

#[test]
fn get_duplicate_strings_invalid_snapshot() {
    let mut proc = McpProcess::start();

    let resp = proc.call_tool(
        1,
        "get_duplicate_strings",
        serde_json::json!({ "snapshot_id": 999 }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("No snapshot found"),
        "expected not-found error, got: {err}"
    );
}
