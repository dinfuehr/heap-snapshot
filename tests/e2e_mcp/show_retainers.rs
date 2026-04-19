use super::*;

// heap-1.heapsnapshot node ids used below:
//   @7271 — system / NativeContext (the utility realm)
//   @3    — (GC roots) synthetic root

#[test]
fn show_retainers() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "show_retainers",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@7271" }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_show_retainers_native_context.txt"
    );
}

#[test]
fn show_retainers_with_depth() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    // depth=1 (default)
    let resp1 = proc.call_tool(
        2,
        "show_retainers",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@7271" }),
    );
    assert_content!(
        get_text(&resp1),
        "expected_mcp_show_retainers_native_context.txt"
    );

    // depth=2
    let resp2 = proc.call_tool(
        3,
        "show_retainers",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@7271", "depth": 2 }),
    );
    assert_content!(
        get_text(&resp2),
        "expected_mcp_show_retainers_native_context_depth2.txt"
    );
}

#[test]
fn show_retainers_with_limit() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "show_retainers",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@7271", "limit": 1 }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_show_retainers_native_context_limit1.txt"
    );
}

#[test]
fn show_retainers_with_offset() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "show_retainers",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@3", "limit": 100 }),
    );
    assert_content!(get_text(&resp), "expected_mcp_show_retainers_gc_roots.txt");
}
