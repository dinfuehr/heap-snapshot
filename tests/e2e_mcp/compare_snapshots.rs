use super::*;

#[test]
fn compare_snapshots_heap2_vs_heap1_new_objects() {
    let mut proc = McpProcess::start();
    let path1 = format!("{}/heap-1.heapsnapshot", test_dir());
    let path2 = format!("{}/heap-2.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path1 }));
    proc.call_tool(2, "load_snapshot", serde_json::json!({ "path": path2 }));

    let resp = proc.call_tool(
        3,
        "compare_snapshots",
        serde_json::json!({ "snapshot_id": 2, "baseline_id": 1 }),
    );
    let text = get_text(&resp);
    assert_eq!(text, "\
10 constructors with changes. Showing 1-10:
Constructor                                           # New  # Deleted  # Delta    Alloc. Size     Freed Size     Size Delta
(concatenated string)                                    45         43       +2         +900 B         +860 B          +40 B
(object shape)                                           11          1      +10         +408 B          +20 B         +388 B
(compiled code)                                           7          1       +6         +252 B          +20 B         +232 B
NewObject                                                 2          0       +2         +120 B            0 B         +120 B
(string)                                                  4          4        0          +76 B          +72 B           +4 B
Array                                                     2          0       +2          +32 B            0 B          +32 B
{constructor}                                             1          0       +1          +28 B            0 B          +28 B
InitialObject                                             0          2       \u{2212}2            0 B         +120 B         \u{2212}120 B
(number)                                                  0          2       \u{2212}2            0 B          +24 B          \u{2212}24 B
{createdAt}                                               0          2       \u{2212}2            0 B          +32 B          \u{2212}32 B");
}

#[test]
fn compare_snapshots_heap2_vs_heap1_deleted_objects() {
    let mut proc = McpProcess::start();
    let path1 = format!("{}/heap-1.heapsnapshot", test_dir());
    let path2 = format!("{}/heap-2.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path1 }));
    proc.call_tool(2, "load_snapshot", serde_json::json!({ "path": path2 }));

    let resp = proc.call_tool(
        3,
        "compare_snapshots",
        serde_json::json!({ "snapshot_id": 2, "baseline_id": 1 }),
    );
    let text = get_text(&resp);
    assert_eq!(text, "\
10 constructors with changes. Showing 1-10:
Constructor                                           # New  # Deleted  # Delta    Alloc. Size     Freed Size     Size Delta
(concatenated string)                                    45         43       +2         +900 B         +860 B          +40 B
(object shape)                                           11          1      +10         +408 B          +20 B         +388 B
(compiled code)                                           7          1       +6         +252 B          +20 B         +232 B
NewObject                                                 2          0       +2         +120 B            0 B         +120 B
(string)                                                  4          4        0          +76 B          +72 B           +4 B
Array                                                     2          0       +2          +32 B            0 B          +32 B
{constructor}                                             1          0       +1          +28 B            0 B          +28 B
InitialObject                                             0          2       \u{2212}2            0 B         +120 B         \u{2212}120 B
(number)                                                  0          2       \u{2212}2            0 B          +24 B          \u{2212}24 B
{createdAt}                                               0          2       \u{2212}2            0 B          +32 B          \u{2212}32 B");
}

#[test]
fn compare_snapshots_heap3_vs_heap1() {
    let mut proc = McpProcess::start();
    let path1 = format!("{}/heap-1.heapsnapshot", test_dir());
    let path3 = format!("{}/heap-3.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path1 }));
    proc.call_tool(2, "load_snapshot", serde_json::json!({ "path": path3 }));

    let resp = proc.call_tool(
        3,
        "compare_snapshots",
        serde_json::json!({ "snapshot_id": 2, "baseline_id": 1 }),
    );
    let text = get_text(&resp);
    assert_eq!(text, "\
11 constructors with changes. Showing 1-11:
Constructor                                           # New  # Deleted  # Delta    Alloc. Size     Freed Size     Size Delta
(concatenated string)                                   150         43     +107          +3 kB         +860 B          +2 kB
(object shape)                                           18          2      +16         +732 B          +76 B         +656 B
(string)                                                 14          4      +10         +276 B          +72 B         +204 B
(compiled code)                                           7          1       +6         +252 B          +20 B         +232 B
NewObject                                                 7          0       +7         +196 B            0 B         +196 B
Array                                                     7          0       +7         +112 B            0 B         +112 B
system / PropertyArray                                    1          0       +1          +32 B            0 B          +32 B
{constructor}                                             1          0       +1          +28 B            0 B          +28 B
InitialObject                                             0          2       \u{2212}2            0 B         +120 B         \u{2212}120 B
(number)                                                  0          2       \u{2212}2            0 B          +24 B          \u{2212}24 B
{createdAt}                                               0          2       \u{2212}2            0 B          +32 B          \u{2212}32 B");
}

#[test]
fn compare_snapshots_identical() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": &path }));
    proc.call_tool(2, "load_snapshot", serde_json::json!({ "path": &path }));

    let resp = proc.call_tool(
        3,
        "compare_snapshots",
        serde_json::json!({ "snapshot_id": 1, "baseline_id": 2 }),
    );
    let text = get_text(&resp);
    assert_eq!(text, "\
0 constructors with changes. Showing 1-0:
Constructor                                           # New  # Deleted  # Delta    Alloc. Size     Freed Size     Size Delta");
}

#[test]
fn compare_snapshots_expand_class() {
    let mut proc = McpProcess::start();
    let path1 = format!("{}/heap-1.heapsnapshot", test_dir());
    let path2 = format!("{}/heap-2.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path1 }));
    proc.call_tool(2, "load_snapshot", serde_json::json!({ "path": path2 }));

    let resp = proc.call_tool(
        3,
        "compare_snapshots",
        serde_json::json!({
            "snapshot_id": 2,
            "baseline_id": 1,
            "class_name": "NewObject"
        }),
    );
    let text = get_text(&resp);
    assert_eq!(text, "\
NewObject: # new: 2, # deleted: 0, # delta: +2, alloc size: +120 B, freed size: 0 B, size delta: +120 B
Showing 1-2 of 2 objects:
  + @21649 (self_size: 60)
  + @21651 (self_size: 60)");
}

#[test]
fn compare_snapshots_expand_class_with_limit() {
    let mut proc = McpProcess::start();
    let path1 = format!("{}/heap-1.heapsnapshot", test_dir());
    let path3 = format!("{}/heap-3.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path1 }));
    proc.call_tool(2, "load_snapshot", serde_json::json!({ "path": path3 }));

    let resp = proc.call_tool(
        3,
        "compare_snapshots",
        serde_json::json!({
            "snapshot_id": 2,
            "baseline_id": 1,
            "class_name": "NewObject",
            "limit": 3
        }),
    );
    let text = get_text(&resp);
    assert_eq!(text, "\
NewObject: # new: 7, # deleted: 0, # delta: +7, alloc size: +196 B, freed size: 0 B, size delta: +196 B
Showing 1-3 of 7 objects:
  + @21649 (self_size: 28)
  + @21651 (self_size: 28)
  + @22093 (self_size: 28)");
}

#[test]
fn compare_snapshots_reversed() {
    let mut proc = McpProcess::start();
    let path1 = format!("{}/heap-1.heapsnapshot", test_dir());
    let path2 = format!("{}/heap-2.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path1 }));
    proc.call_tool(2, "load_snapshot", serde_json::json!({ "path": path2 }));

    let resp = proc.call_tool(
        3,
        "compare_snapshots",
        serde_json::json!({ "snapshot_id": 1, "baseline_id": 2 }),
    );
    let text = get_text(&resp);
    assert_eq!(text, "\
10 constructors with changes. Showing 1-10:
Constructor                                           # New  # Deleted  # Delta    Alloc. Size     Freed Size     Size Delta
(concatenated string)                                    43         45       \u{2212}2         +860 B         +900 B          \u{2212}40 B
InitialObject                                             2          0       +2         +120 B            0 B         +120 B
(string)                                                  4          4        0          +72 B          +76 B           \u{2212}4 B
{createdAt}                                               2          0       +2          +32 B            0 B          +32 B
(number)                                                  2          0       +2          +24 B            0 B          +24 B
(object shape)                                            1         11      \u{2212}10          +20 B         +408 B         \u{2212}388 B
(compiled code)                                           1          7       \u{2212}6          +20 B         +252 B         \u{2212}232 B
NewObject                                                 0          2       \u{2212}2            0 B         +120 B         \u{2212}120 B
{constructor}                                             0          1       \u{2212}1            0 B          +28 B          \u{2212}28 B
Array                                                     0          2       \u{2212}2            0 B          +32 B          \u{2212}32 B");
}

#[test]
fn compare_snapshots_invalid_class_name() {
    let mut proc = McpProcess::start();
    let path1 = format!("{}/heap-1.heapsnapshot", test_dir());
    let path2 = format!("{}/heap-2.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path1 }));
    proc.call_tool(2, "load_snapshot", serde_json::json!({ "path": path2 }));

    let resp = proc.call_tool(
        3,
        "compare_snapshots",
        serde_json::json!({
            "snapshot_id": 2,
            "baseline_id": 1,
            "class_name": "NoSuchConstructor"
        }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("No diff entry for constructor"),
        "expected not-found error, got: {err}"
    );
}

#[test]
fn compare_snapshots_invalid_snapshot_id() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "compare_snapshots",
        serde_json::json!({ "snapshot_id": 1, "baseline_id": 99 }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("No snapshot found with id 99"),
        "expected missing snapshot error, got: {err}"
    );
}
