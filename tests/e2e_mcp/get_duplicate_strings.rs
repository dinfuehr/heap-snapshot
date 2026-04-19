use super::*;

fn load_heap3(proc: &mut McpProcess) {
    let path = format!("{}/heap-3.heapsnapshot", test_dir());
    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));
}

#[test]
fn get_duplicate_strings_basic() {
    let mut proc = McpProcess::start();
    load_heap3(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_duplicate_strings",
        serde_json::json!({ "snapshot_id": 1 }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_get_duplicate_strings_default.txt"
    );
}

#[test]
fn get_duplicate_strings_offset_limit_slices_disjointly() {
    let mut proc = McpProcess::start();
    load_heap3(&mut proc);

    let resp1 = proc.call_tool(
        2,
        "get_duplicate_strings",
        serde_json::json!({ "snapshot_id": 1, "offset": 0, "limit": 2 }),
    );
    assert_content!(
        get_text(&resp1),
        "expected_mcp_get_duplicate_strings_offset0_limit2.txt"
    );

    let resp2 = proc.call_tool(
        3,
        "get_duplicate_strings",
        serde_json::json!({ "snapshot_id": 1, "offset": 2, "limit": 2 }),
    );
    assert_content!(
        get_text(&resp2),
        "expected_mcp_get_duplicate_strings_offset2_limit2.txt"
    );
}

#[test]
fn get_duplicate_strings_offset_past_end_clamped() {
    let mut proc = McpProcess::start();
    load_heap3(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_duplicate_strings",
        serde_json::json!({ "snapshot_id": 1, "offset": 99999 }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_get_duplicate_strings_offset_past_end.txt"
    );
}

#[test]
fn get_duplicate_strings_show_object_ids_true_emits_ids() {
    let mut proc = McpProcess::start();
    load_heap3(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_duplicate_strings",
        serde_json::json!({ "snapshot_id": 1, "limit": 3, "show_object_ids": true }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_get_duplicate_strings_limit3_show_ids.txt"
    );
}

#[test]
fn get_duplicate_strings_show_object_ids_default_hides_ids() {
    let mut proc = McpProcess::start();
    load_heap3(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_duplicate_strings",
        serde_json::json!({ "snapshot_id": 1, "limit": 3 }),
    );
    assert_content!(
        get_text(&resp),
        "expected_mcp_get_duplicate_strings_limit3.txt"
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
