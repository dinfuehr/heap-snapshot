use super::*;

#[test]
fn get_containment() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "get_containment",
        serde_json::json!({ "snapshot_id": 1 }),
    );
    let text = get_text(&resp);
    assert_eq!(
        text,
        "\
System roots:
  [1] @3 (GC roots) (self_size: 0, retained_size: 128420)
  [2] @2 C++ Persistent roots (self_size: 0, retained_size: 0)
  [3] @4 C++ CrossThreadPersistent roots (self_size: 0, retained_size: 0)
  [4] @6 C++ native stack roots (self_size: 0, retained_size: 0)

(GC roots) children:
  [1] @5 (Bootstrapper) (self_size: 0, retained_size: 0, children: 0)
  [2] @7 (Builtins) (self_size: 0, retained_size: 0, children: 2426)
  [3] @9 (Client heap) (self_size: 0, retained_size: 0, children: 0)
  [4] @11 (Code flusher) (self_size: 0, retained_size: 0, children: 0)
  [5] @13 (Compilation cache) (self_size: 0, retained_size: 0, children: 5)
  [6] @15 (Debugger) (self_size: 0, retained_size: 0, children: 0)
  [7] @17 (Extensions) (self_size: 0, retained_size: 0, children: 1)
  [8] @19 (Eternal handles) (self_size: 0, retained_size: 0, children: 0)
  [9] @21 (External strings) (self_size: 0, retained_size: 0, children: 0)
  [10] @23 (Global handles) (self_size: 0, retained_size: 0, children: 4)
  [11] @25 (Handle scope) (self_size: 0, retained_size: 0, children: 226)
  [12] @27 (Identity map) (self_size: 0, retained_size: 0, children: 0)
  [13] @29 (Micro tasks) (self_size: 0, retained_size: 0, children: 0)
  [14] @31 (Read-only roots) (self_size: 0, retained_size: 0, children: 1049)
  [15] @33 (Relocatable) (self_size: 0, retained_size: 0, children: 0)
  [16] @35 (Retain maps) (self_size: 0, retained_size: 0, children: 0)
  [17] @37 (Shareable object cache) (self_size: 0, retained_size: 0, children: 1)
  [18] @39 (SharedStruct type registry) (self_size: 0, retained_size: 0, children: 0)
  [19] @41 (Smi roots) (self_size: 0, retained_size: 0, children: 0)
  [20] @43 (Stack roots) (self_size: 0, retained_size: 28, children: 21)
  [21] @45 (Startup object cache) (self_size: 0, retained_size: 96, children: 64)
  [22] @47 (Internalized strings) (self_size: 0, retained_size: 0, children: 1769)
  [23] @49 (Strong root list) (self_size: 0, retained_size: 6764, children: 99)
  [24] @51 (Strong roots) (self_size: 0, retained_size: 0, children: 0)
  [25] @53 (Thread manager) (self_size: 0, retained_size: 0, children: 0)
  [26] @55 (Traced handles) (self_size: 0, retained_size: 0, children: 0)
  [27] @57 (Weak roots) (self_size: 0, retained_size: 0, children: 0)
  [28] @59 (Write barrier) (self_size: 0, retained_size: 0, children: 0)"
    );
}
