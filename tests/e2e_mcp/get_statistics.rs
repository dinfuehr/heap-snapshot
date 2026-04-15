use super::*;

#[test]
fn get_statistics() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(2, "get_statistics", serde_json::json!({ "snapshot_id": 1 }));
    let text = get_text(&resp);
    let expected = "\
10653 nodes, 128420 bytes total
  V8 heap:      107880 bytes
  Native:       20540 bytes
  Code:         8336 bytes
  Strings:      5836 bytes
  JS arrays:    64 bytes
  Extra native: 0 bytes
  Typed arrays: 0 bytes
  System:       0 bytes
  Unreachable:  0 bytes (0 objects)

Native Context Attribution:
  [utility] #0 @7165: 121532 bytes
  Shared: 0 bytes
  Unattributed: 6888 bytes";
    assert_eq!(text, expected);
}
