use super::*;

#[test]
fn show() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1" }),
    );
    let text = get_text(&resp);
    assert_eq!(
        text,
        r#"Object @1:  (type: synthetic, self_size: 0)
  --[element "1"]--> @3 (GC roots) (type: synthetic, self_size: 0)
  --[element "2"]--> @2 C++ Persistent roots (type: synthetic, self_size: 0)
  --[element "3"]--> @4 C++ CrossThreadPersistent roots (type: synthetic, self_size: 0)
  --[element "4"]--> @6 C++ native stack roots (type: synthetic, self_size: 0)"#
    );
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
    let text = get_text(&resp);
    assert_eq!(
        text,
        r#"Object @1:  (type: synthetic, self_size: 0)
  --[element "1"]--> @3 (GC roots) (type: synthetic, self_size: 0)
  --[element "2"]--> @2 C++ Persistent roots (type: synthetic, self_size: 0)
  --[element "3"]--> @4 C++ CrossThreadPersistent roots (type: synthetic, self_size: 0)
  --[element "4"]--> @6 C++ native stack roots (type: synthetic, self_size: 0)"#
    );
}

#[test]
fn show_with_depth() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    // depth=1 (default) should only have one level of indentation
    let resp1 = proc.call_tool(
        2,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1" }),
    );
    let text1 = get_text(&resp1);
    assert_eq!(
        text1,
        r#"Object @1:  (type: synthetic, self_size: 0)
  --[element "1"]--> @3 (GC roots) (type: synthetic, self_size: 0)
  --[element "2"]--> @2 C++ Persistent roots (type: synthetic, self_size: 0)
  --[element "3"]--> @4 C++ CrossThreadPersistent roots (type: synthetic, self_size: 0)
  --[element "4"]--> @6 C++ native stack roots (type: synthetic, self_size: 0)"#
    );

    // depth=2 should have nested edges (double indentation)
    let resp2 = proc.call_tool(
        3,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1", "depth": 2 }),
    );
    let text2 = get_text(&resp2);
    assert_eq!(
        text2,
        r#"Object @1:  (type: synthetic, self_size: 0)
  --[element "1"]--> @3 (GC roots) (type: synthetic, self_size: 0)
    --[element "1"]--> @5 (Bootstrapper) (type: synthetic, self_size: 0)
    --[element "2"]--> @7 (Builtins) (type: synthetic, self_size: 0)
    --[element "3"]--> @9 (Client heap) (type: synthetic, self_size: 0)
    --[element "4"]--> @11 (Code flusher) (type: synthetic, self_size: 0)
    --[element "5"]--> @13 (Compilation cache) (type: synthetic, self_size: 0)
    --[element "6"]--> @15 (Debugger) (type: synthetic, self_size: 0)
    --[element "7"]--> @17 (Extensions) (type: synthetic, self_size: 0)
    --[element "8"]--> @19 (Eternal handles) (type: synthetic, self_size: 0)
    --[element "9"]--> @21 (External strings) (type: synthetic, self_size: 0)
    --[element "10"]--> @23 (Global handles) (type: synthetic, self_size: 0)
    --[element "11"]--> @25 (Handle scope) (type: synthetic, self_size: 0)
    --[element "12"]--> @27 (Identity map) (type: synthetic, self_size: 0)
    --[element "13"]--> @29 (Micro tasks) (type: synthetic, self_size: 0)
    --[element "14"]--> @31 (Read-only roots) (type: synthetic, self_size: 0)
    --[element "15"]--> @33 (Relocatable) (type: synthetic, self_size: 0)
    --[element "16"]--> @35 (Retain maps) (type: synthetic, self_size: 0)
    --[element "17"]--> @37 (Shareable object cache) (type: synthetic, self_size: 0)
    --[element "18"]--> @39 (SharedStruct type registry) (type: synthetic, self_size: 0)
    --[element "19"]--> @41 (Smi roots) (type: synthetic, self_size: 0)
    --[element "20"]--> @43 (Stack roots) (type: synthetic, self_size: 0)
    --[element "21"]--> @45 (Startup object cache) (type: synthetic, self_size: 0)
    --[element "22"]--> @47 (Internalized strings) (type: synthetic, self_size: 0)
    --[element "23"]--> @49 (Strong root list) (type: synthetic, self_size: 0)
    --[element "24"]--> @51 (Strong roots) (type: synthetic, self_size: 0)
    --[element "25"]--> @53 (Thread manager) (type: synthetic, self_size: 0)
    --[element "26"]--> @55 (Traced handles) (type: synthetic, self_size: 0)
    --[element "27"]--> @57 (Weak roots) (type: synthetic, self_size: 0)
    --[element "28"]--> @59 (Write barrier) (type: synthetic, self_size: 0)
  --[element "2"]--> @2 C++ Persistent roots (type: synthetic, self_size: 0)
  --[element "3"]--> @4 C++ CrossThreadPersistent roots (type: synthetic, self_size: 0)
  --[element "4"]--> @6 C++ native stack roots (type: synthetic, self_size: 0)"#
    );
}

#[test]
fn show_with_limit() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    let resp = proc.call_tool(
        2,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1", "limit": 2 }),
    );
    let text = get_text(&resp);
    assert_eq!(
        text,
        r#"Object @1:  (type: synthetic, self_size: 0)
  --[element "1"]--> @3 (GC roots) (type: synthetic, self_size: 0)
  --[element "2"]--> @2 C++ Persistent roots (type: synthetic, self_size: 0)
  (1-2 of 4 children shown)"#
    );
}

#[test]
fn show_with_offset() {
    let mut proc = McpProcess::start();
    load_heap1(&mut proc);

    // Get all children first
    let resp_all = proc.call_tool(
        2,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1" }),
    );
    let text_all = get_text(&resp_all);
    assert_eq!(
        text_all,
        r#"Object @1:  (type: synthetic, self_size: 0)
  --[element "1"]--> @3 (GC roots) (type: synthetic, self_size: 0)
  --[element "2"]--> @2 C++ Persistent roots (type: synthetic, self_size: 0)
  --[element "3"]--> @4 C++ CrossThreadPersistent roots (type: synthetic, self_size: 0)
  --[element "4"]--> @6 C++ native stack roots (type: synthetic, self_size: 0)"#
    );

    // Get with offset=1
    let resp_offset = proc.call_tool(
        3,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1", "offset": 1 }),
    );
    let text_offset = get_text(&resp_offset);
    assert_eq!(
        text_offset,
        r#"Object @1:  (type: synthetic, self_size: 0)
  --[element "2"]--> @2 C++ Persistent roots (type: synthetic, self_size: 0)
  --[element "3"]--> @4 C++ CrossThreadPersistent roots (type: synthetic, self_size: 0)
  --[element "4"]--> @6 C++ native stack roots (type: synthetic, self_size: 0)"#
    );
}
