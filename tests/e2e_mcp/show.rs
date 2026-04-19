use super::*;

// All of these tests operate on @1 (synthetic root) or @3 (GC roots), which
// are stable across V8 snapshot revisions. @1 has a single child (@3) in the
// current format; @3 has many children and is convenient for exercising depth,
// offset, and limit.

#[test]
fn show() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1" }),
    );
    assert_content!(get_text(&resp), "expected_mcp_show_root.txt");
}

#[test]
fn show_invalid_object() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@999999999" }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("No object found"),
        "expected not-found error, got: {err}"
    );
}

#[test]
fn show_invalid_format() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "not_a_number" }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("Invalid object id"),
        "expected invalid id error, got: {err}"
    );
}

#[test]
fn show_without_at_prefix() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "1" }),
    );
    // Same object as "@1" above — output should be identical.
    assert_content!(get_text(&resp), "expected_mcp_show_root.txt");
}

#[test]
fn show_with_depth() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    // depth=1 (default): direct edges only.
    let resp1 = proc.call_tool(
        2,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@3" }),
    );
    assert_content!(get_text(&resp1), "expected_mcp_show_gc_roots.txt");

    // depth=2: nested edges appear with double indentation.
    let resp2 = proc.call_tool(
        3,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@3", "depth": 2 }),
    );
    assert_content!(get_text(&resp2), "expected_mcp_show_gc_roots_depth2.txt");
}

#[test]
fn show_with_limit() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@3", "limit": 2 }),
    );
    assert_content!(get_text(&resp), "expected_mcp_show_gc_roots_limit2.txt");
}

#[test]
fn show_with_offset() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    // Without offset, first edge is element "1".
    let resp_all = proc.call_tool(
        2,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@3", "limit": 3 }),
    );
    assert_content!(get_text(&resp_all), "expected_mcp_show_gc_roots_limit3.txt");

    // With offset=1, first edge should be element "2".
    let resp_offset = proc.call_tool(
        3,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@3", "offset": 1, "limit": 3 }),
    );
    assert_content!(
        get_text(&resp_offset),
        "expected_mcp_show_gc_roots_offset1_limit3.txt"
    );
}
