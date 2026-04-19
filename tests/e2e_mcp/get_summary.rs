use super::*;

#[test]
fn get_summary() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(2, "get_summary", serde_json::json!({ "snapshot_id": 1 }));
    assert_content!(get_text(&resp), "expected_mcp_get_summary_default.txt");
}

#[test]
fn get_summary_expand_constructor() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_summary",
        serde_json::json!({ "snapshot_id": 1, "class_name": "Function", "limit": 3 }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_get_summary_function_limit3.txt"
    );
}

#[test]
fn get_summary_expand_sorted_by_retained_size() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_summary",
        serde_json::json!({ "snapshot_id": 1, "class_name": "Function" }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_get_summary_function_default.txt"
    );
}

#[test]
fn get_summary_expand_invalid_constructor() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_summary",
        serde_json::json!({ "snapshot_id": 1, "class_name": "NoSuchConstructor" }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("No constructor group"),
        "expected not-found error, got: {err}"
    );
}
