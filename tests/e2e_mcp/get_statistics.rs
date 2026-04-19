use super::*;

#[test]
fn get_statistics() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(2, "get_statistics", serde_json::json!({ "snapshot_id": 1 }));
    assert_content!(get_text(&resp), "expected_mcp_get_statistics.txt");
}
