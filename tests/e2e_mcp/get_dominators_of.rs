use super::*;

#[test]
fn get_dominators_of() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_dominators_of",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@7165" }),
    );
    let text = get_text(&resp);
    assert_eq!(text, "\
Dominator chain for @7165: system / NativeContext (type: native, self_size: 1240, retained_size: 23708)
  dominated by @3 (GC roots) (type: synthetic, self_size: 0, retained_size: 128420)");
}
