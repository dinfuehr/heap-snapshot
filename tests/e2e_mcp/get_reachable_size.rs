use super::*;

#[test]
fn get_reachable_size() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_reachable_size",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1" }),
    );
    let text = get_text(&resp);
    assert_eq!(
        text,
        "\
Reachable size from @1 (): 128420 bytes
1 native contexts reached:
  @7165 [utility] #0 @7165"
    );
}

#[test]
fn get_reachable_size_reaches_native_context() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    // @25 is (Handle scope) which reaches native context @7165
    let resp = proc.call_tool(
        2,
        "get_reachable_size",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@25" }),
    );
    let text = get_text(&resp);
    assert_eq!(
        text,
        "\
Reachable size from @25 ((Handle scope)): 121532 bytes
1 native contexts reached:
  @7165 [utility] #0 @7165"
    );
}

#[test]
fn get_reachable_size_invalid_object() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_reachable_size",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@999999999" }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("No object found"),
        "expected not-found error, got: {err}"
    );
}
