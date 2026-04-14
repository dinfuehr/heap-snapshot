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
    assert_eq!(
        text,
        r#"48 duplicate string groups, 1196 bytes wasted total
Showing entries 0..20:

"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx..." x39 (instance_size: 20, total: 780, wasted: 760)
"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" x3 (instance_size: 20, total: 60, wasted: 40)
"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" x3 (instance_size: 20, total: 60, wasted: 40)
"xxxxxxxxxxxxxxxx" x3 (instance_size: 20, total: 60, wasted: 40)
"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx..." x3 (instance_size: 20, total: 60, wasted: 40)
"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx..." x3 (instance_size: 20, total: 60, wasted: 40)
"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx..." x3 (instance_size: 20, total: 60, wasted: 40)
"xxxxxxxx" x3 (instance_size: 20, total: 60, wasted: 40)
"WebAssembly.Exception" x2 (instance_size: 0, total: 36, wasted: 36)
"WebAssembly.Module" x2 (instance_size: 0, total: 32, wasted: 32)
"get disposed" x2 (instance_size: 24, total: 48, wasted: 24)
"WebAssembly" x2 (instance_size: 24, total: 48, wasted: 24)
"global" x2 (instance_size: 0, total: 20, wasted: 20)
"Worker" x2 (instance_size: 20, total: 40, wasted: 20)
"NaN" x5 (instance_size: 0, total: 0, wasted: 0)
"1771913344114" x3 (instance_size: 0, total: 0, wasted: 0)
"0" x3 (instance_size: 0, total: 0, wasted: 0)
"-Infinity" x3 (instance_size: 0, total: 0, wasted: 0)
"Infinity" x3 (instance_size: 0, total: 0, wasted: 0)
"19" x2 (instance_size: 0, total: 0, wasted: 0)

Use offset=20 to see more entries (28 remaining)."#
    );
}

#[test]
fn get_duplicate_strings_pagination() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    // Request only 2 entries
    let resp = proc.call_tool(
        2,
        "get_duplicate_strings",
        serde_json::json!({ "snapshot_id": 1, "limit": 2 }),
    );
    let text = get_text(&resp);
    assert_eq!(
        text,
        r#"48 duplicate string groups, 1196 bytes wasted total
Showing entries 0..2:

"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx..." x39 (instance_size: 20, total: 780, wasted: 760)
"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" x3 (instance_size: 20, total: 60, wasted: 40)

Use offset=2 to see more entries (46 remaining)."#
    );

    // Request with offset
    let resp2 = proc.call_tool(
        3,
        "get_duplicate_strings",
        serde_json::json!({ "snapshot_id": 1, "offset": 2, "limit": 2 }),
    );
    let text2 = get_text(&resp2);
    assert_eq!(
        text2,
        r#"48 duplicate string groups, 1196 bytes wasted total
Showing entries 2..4:

"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" x3 (instance_size: 20, total: 60, wasted: 40)
"xxxxxxxxxxxxxxxx" x3 (instance_size: 20, total: 60, wasted: 40)

Use offset=4 to see more entries (44 remaining)."#
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
