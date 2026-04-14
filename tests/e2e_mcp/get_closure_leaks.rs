use super::*;

#[test]
fn get_closure_leaks_no_leaks() {
    let mut proc = McpProcess::start();
    let path = format!("{}/closures.heapsnapshot", test_dir());
    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "get_closure_leaks",
        serde_json::json!({ "snapshot_id": 1 }),
    );
    let text = get_text(&resp);
    assert_eq!(text, "No closure leaks detected.");
}

#[test]
fn get_closure_leaks_show_incomplete() {
    let mut proc = McpProcess::start();
    let path = format!("{}/closures.heapsnapshot", test_dir());
    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "get_closure_leaks",
        serde_json::json!({ "snapshot_id": 1, "show_incomplete": true }),
    );
    let text = get_text(&resp);
    assert_eq!(
        text,
        "\
@124649 (retained: 168 B)  vars: [innerOnly]
  (incomplete: script @7653 source is missing or could not be parsed)
@124645 (retained: 148 B)  vars: [shared]
  (incomplete: script @7653 source is missing or could not be parsed)
@7295 (retained: 68 B)  vars: [Emitter, counter, emitter, fns, greeter, nested, secret]
  (incomplete: script @7653 source is missing or could not be parsed)
@124639 (retained: 44 B)  vars: [count]
  (incomplete: script @7653 source is missing or could not be parsed)
@124643 (retained: 28 B)  vars: [greeting, prefix, punctuation]
  (incomplete: script @7653 source is missing or could not be parsed)
@124665 (retained: 20 B)  vars: [hidden]
  (incomplete: script @7653 source is missing or could not be parsed)

6 contexts with unused variables"
    );
}

#[test]
fn get_closure_leaks_invalid_snapshot() {
    let mut proc = McpProcess::start();
    let resp = proc.call_tool(
        1,
        "get_closure_leaks",
        serde_json::json!({ "snapshot_id": 999 }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("No snapshot found"),
        "expected not-found error, got: {err}"
    );
}
