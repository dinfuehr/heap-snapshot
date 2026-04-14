mod common;

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};

fn test_dir() -> &'static str {
    common::test_dir()
}

fn mcp_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_heap-snapshot"));
    cmd.arg("mcp");
    cmd
}

/// A running MCP server process that supports multi-round request/response.
struct McpProcess {
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    #[allow(dead_code)]
    child: Child,
}

impl McpProcess {
    fn start() -> Self {
        let mut child = mcp_bin()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to start heap-snapshot-mcp");

        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut proc = McpProcess {
            stdin,
            stdout,
            child,
        };

        // Perform initialize handshake
        proc.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "0.0.1" }
            }
        }));
        proc.recv(); // consume initialize response
        proc.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }));

        proc
    }

    fn send(&mut self, msg: &serde_json::Value) {
        serde_json::to_writer(&mut self.stdin, msg).unwrap();
        writeln!(self.stdin).unwrap();
        self.stdin.flush().unwrap();
    }

    fn recv(&mut self) -> serde_json::Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(line.trim()).expect("invalid JSON response")
    }

    fn call_tool(&mut self, id: u64, name: &str, args: serde_json::Value) -> serde_json::Value {
        self.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": args
            }
        }));
        self.recv()
    }
}

fn get_text(response: &serde_json::Value) -> String {
    response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

fn get_error_message(response: &serde_json::Value) -> String {
    response["error"]["message"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[test]
fn load_snapshot() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    let resp = proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));
    let text = get_text(&resp);
    assert!(
        text.contains("snapshot_id: 1"),
        "expected snapshot_id in response, got: {text}"
    );
    assert!(
        text.contains("nodes"),
        "expected node count in response, got: {text}"
    );
}

#[test]
fn load_and_close_snapshot() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    let resp = proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));
    let text = get_text(&resp);
    assert!(text.contains("snapshot_id: 1"), "load failed: {text}");

    let resp = proc.call_tool(2, "close_snapshot", serde_json::json!({ "snapshot_id": 1 }));
    let text = get_text(&resp);
    assert!(
        text.contains("Closed snapshot 1"),
        "expected close confirmation, got: {text}"
    );
}

#[test]
fn close_nonexistent_snapshot() {
    let mut proc = McpProcess::start();

    let resp = proc.call_tool(
        1,
        "close_snapshot",
        serde_json::json!({ "snapshot_id": 999 }),
    );
    let text = get_text(&resp);
    assert!(
        text.contains("No snapshot found"),
        "expected not-found message, got: {text}"
    );
}

#[test]
fn load_nonexistent_file() {
    let mut proc = McpProcess::start();

    let resp = proc.call_tool(
        1,
        "load_snapshot",
        serde_json::json!({ "path": "/nonexistent/file.heapsnapshot" }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("Failed to open"),
        "expected open error, got: {err}"
    );
}

#[test]
fn show() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1" }),
    );
    let text = get_text(&resp);
    assert!(
        text.contains("Object @1"),
        "expected object header, got: {text}"
    );
    assert!(
        text.contains("--["),
        "expected at least one edge, got: {text}"
    );
}

#[test]
fn show_invalid_object() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

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
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

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
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "1" }),
    );
    let text = get_text(&resp);
    assert!(
        text.contains("Object @1"),
        "expected object_id without @ prefix to work, got: {text}"
    );
}

#[test]
fn show_with_depth() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    // depth=1 (default) should only have one level of indentation
    let resp1 = proc.call_tool(
        2,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1" }),
    );
    let text1 = get_text(&resp1);
    let depth1_lines: Vec<_> = text1.lines().filter(|l| l.starts_with("    --[")).collect();
    assert!(
        depth1_lines.is_empty(),
        "depth=1 should not have nested edges, got: {text1}"
    );

    // depth=2 should have nested edges (double indentation)
    let resp2 = proc.call_tool(
        3,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1", "depth": 2 }),
    );
    let text2 = get_text(&resp2);
    let depth2_lines: Vec<_> = text2.lines().filter(|l| l.starts_with("    --[")).collect();
    assert!(
        !depth2_lines.is_empty(),
        "depth=2 should have nested edges, got: {text2}"
    );
}

#[test]
fn show_with_limit() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1", "limit": 2 }),
    );
    let text = get_text(&resp);
    let edge_lines: Vec<_> = text.lines().filter(|l| l.starts_with("  --[")).collect();
    assert!(
        edge_lines.len() <= 2,
        "expected at most 2 edges with limit=2, got: {}",
        edge_lines.len()
    );
    assert!(
        text.contains("children shown"),
        "expected truncation message, got: {text}"
    );
}

#[test]
fn show_with_offset() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    // Get all children first
    let resp_all = proc.call_tool(
        2,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1" }),
    );
    let text_all = get_text(&resp_all);
    let all_edges: Vec<_> = text_all
        .lines()
        .filter(|l| l.starts_with("  --["))
        .collect();

    // Get with offset=1
    let resp_offset = proc.call_tool(
        3,
        "show",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1", "offset": 1 }),
    );
    let text_offset = get_text(&resp_offset);
    let offset_edges: Vec<_> = text_offset
        .lines()
        .filter(|l| l.starts_with("  --["))
        .collect();

    assert!(
        offset_edges.len() < all_edges.len(),
        "offset should reduce the number of edges shown"
    );
    // The first edge with offset=1 should be the second edge without offset
    if all_edges.len() > 1 {
        assert_eq!(
            offset_edges[0], all_edges[1],
            "offset=1 should skip the first edge"
        );
    }
}

#[test]
fn show_retainers() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "show_retainers",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@7165" }),
    );
    let text = get_text(&resp);
    assert!(
        text.contains("Object @7165"),
        "expected object header, got: {text}"
    );
    assert!(
        text.contains("<--["),
        "expected at least one retainer edge, got: {text}"
    );
}

#[test]
fn show_retainers_with_depth() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    // depth=1 should not have nested retainers
    let resp1 = proc.call_tool(
        2,
        "show_retainers",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@7165" }),
    );
    let text1 = get_text(&resp1);
    let nested: Vec<_> = text1
        .lines()
        .filter(|l| l.starts_with("    <--["))
        .collect();
    assert!(
        nested.is_empty(),
        "depth=1 should not have nested retainers, got: {text1}"
    );

    // depth=2 should have nested retainers
    let resp2 = proc.call_tool(
        3,
        "show_retainers",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@7165", "depth": 2 }),
    );
    let text2 = get_text(&resp2);
    let nested2: Vec<_> = text2
        .lines()
        .filter(|l| l.starts_with("    <--["))
        .collect();
    assert!(
        !nested2.is_empty(),
        "depth=2 should have nested retainers, got: {text2}"
    );
}

#[test]
fn show_retainers_with_limit() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "show_retainers",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@7165", "limit": 1 }),
    );
    let text = get_text(&resp);
    let retainer_lines: Vec<_> = text.lines().filter(|l| l.starts_with("  <--[")).collect();
    assert!(
        retainer_lines.len() <= 1,
        "expected at most 1 retainer with limit=1, got: {}",
        retainer_lines.len()
    );
}

#[test]
fn show_retainers_with_offset() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    // Use (GC roots) @3 which has many children that retain objects
    let resp_all = proc.call_tool(
        2,
        "show_retainers",
        // @25 is (Handle scope), retained by (GC roots) — use @3 (GC roots)
        // which itself is retained by the synthetic root via multiple edges.
        // Instead, find an object with multiple retainers: use show to get
        // a child of (GC roots) and then check its retainers.
        serde_json::json!({ "snapshot_id": 1, "object_id": "@3", "limit": 100 }),
    );
    let text_all = get_text(&resp_all);
    let all_retainers: Vec<_> = text_all
        .lines()
        .filter(|l| l.starts_with("  <--["))
        .collect();

    // Only test offset behavior if the object has multiple retainers
    if all_retainers.len() >= 2 {
        let resp_offset = proc.call_tool(
            3,
            "show_retainers",
            serde_json::json!({ "snapshot_id": 1, "object_id": "@3", "offset": 1 }),
        );
        let text_offset = get_text(&resp_offset);
        let offset_retainers: Vec<_> = text_offset
            .lines()
            .filter(|l| l.starts_with("  <--["))
            .collect();

        assert!(
            offset_retainers.len() < all_retainers.len(),
            "offset should reduce the number of retainers shown"
        );
        assert_eq!(
            offset_retainers[0], all_retainers[1],
            "offset=1 should skip the first retainer"
        );
    }
}

#[test]
fn get_native_contexts() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "get_native_contexts",
        serde_json::json!({ "snapshot_id": 1 }),
    );
    let text = get_text(&resp);
    assert!(
        text.contains("native contexts:"),
        "expected contexts header, got: {text}"
    );
    assert!(
        text.contains("@7165"),
        "expected native context @7165, got: {text}"
    );
    assert!(
        text.contains("utility"),
        "expected utility label, got: {text}"
    );
}

#[test]
fn get_native_contexts_invalid_snapshot() {
    let mut proc = McpProcess::start();

    let resp = proc.call_tool(
        1,
        "get_native_contexts",
        serde_json::json!({ "snapshot_id": 999 }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("No snapshot found"),
        "expected not-found error, got: {err}"
    );
}

#[test]
fn get_reachable_size() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "get_reachable_size",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1" }),
    );
    let text = get_text(&resp);
    assert!(
        text.contains("Reachable size from @1"),
        "expected reachable size header, got: {text}"
    );
    assert!(text.contains("bytes"), "expected byte count, got: {text}");
    assert!(
        text.contains("native contexts"),
        "expected native contexts section, got: {text}"
    );
}

#[test]
fn get_reachable_size_reaches_native_context() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    // @25 is (Handle scope) which reaches native context @7165
    let resp = proc.call_tool(
        2,
        "get_reachable_size",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@25" }),
    );
    let text = get_text(&resp);
    assert!(
        text.contains("Reachable size from @25"),
        "expected reachable size header, got: {text}"
    );
    assert!(
        text.contains("native contexts reached:"),
        "expected reached contexts, got: {text}"
    );
    assert!(
        text.contains("@7165"),
        "expected native context @7165 to be reached, got: {text}"
    );
}

#[test]
fn get_reachable_size_invalid_object() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

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

#[test]
fn get_retaining_paths_error_on_limits() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    // Use very small limits to trigger truncation
    let resp = proc.call_tool(
        2,
        "get_retaining_paths",
        serde_json::json!({
            "snapshot_id": 1,
            "object_id": "@7165",
            "max_depth": 1,
            "max_nodes": 1
        }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("Retaining paths for @7165"),
        "expected retaining paths header in error, got: {err}"
    );
}

#[test]
fn get_statistics() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

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
  [utility] $0 @7165: 121532 bytes
  Shared: 0 bytes
  Unattributed: 6888 bytes";
    assert_eq!(text, expected);
}

#[test]
fn get_summary() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(2, "get_summary", serde_json::json!({ "snapshot_id": 1 }));
    let text = get_text(&resp);
    assert!(
        text.contains("Constructor"),
        "expected header row, got: {text}"
    );
    assert!(
        text.contains("Shallow size"),
        "expected shallow size column, got: {text}"
    );
    assert!(
        text.contains("Retained size"),
        "expected retained size column, got: {text}"
    );
    // The test snapshot should have at least some constructors
    assert!(
        text.lines().count() > 1,
        "expected at least one data row, got: {text}"
    );
}

#[test]
fn get_summary_expand_constructor() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "get_summary",
        serde_json::json!({ "snapshot_id": 1, "class_name": "Function", "limit": 3 }),
    );
    let text = get_text(&resp);
    assert!(
        text.contains("Function:"),
        "expected constructor header, got: {text}"
    );
    assert!(
        text.contains("objects"),
        "expected object count, got: {text}"
    );
    assert!(text.contains("@"), "expected object ids, got: {text}");
    // Should respect limit
    let object_lines: Vec<_> = text.lines().filter(|l| l.contains("  @")).collect();
    assert!(
        object_lines.len() <= 3,
        "expected at most 3 objects, got: {}",
        object_lines.len()
    );
}

#[test]
fn get_containment() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "get_containment",
        serde_json::json!({ "snapshot_id": 1 }),
    );
    let text = get_text(&resp);
    assert!(
        text.contains("System roots:"),
        "expected system roots header, got: {text}"
    );
    assert!(
        text.contains("(GC roots) children:"),
        "expected GC roots children header, got: {text}"
    );
    assert!(
        text.contains("(GC roots)"),
        "expected (GC roots) as a system root, got: {text}"
    );
    // GC roots should have at least one child (root category)
    let gc_children_section = text.split("(GC roots) children:").nth(1).unwrap();
    let child_lines: Vec<_> = gc_children_section
        .lines()
        .filter(|l| l.starts_with("  ["))
        .collect();
    assert!(
        !child_lines.is_empty(),
        "expected at least one (GC roots) child, got: {text}"
    );
    // System roots should only contain system roots, not user roots
    let system_section = text.split("(GC roots) children:").next().unwrap();
    let system_lines: Vec<_> = system_section
        .lines()
        .filter(|l| l.starts_with("  ["))
        .collect();
    assert!(
        !system_lines.is_empty(),
        "expected at least one system root, got: {text}"
    );
}

#[test]
fn get_summary_expand_sorted_by_retained_size() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "get_summary",
        serde_json::json!({ "snapshot_id": 1, "class_name": "Function" }),
    );
    let text = get_text(&resp);

    let retained_sizes: Vec<f64> = text
        .lines()
        .filter_map(|line| {
            let marker = "retained_size: ";
            let start = line.find(marker)? + marker.len();
            let end = start + line[start..].find(')')?;
            line[start..end].parse().ok()
        })
        .collect();

    assert!(
        retained_sizes.len() >= 2,
        "expected at least 2 objects to compare, got: {}",
        retained_sizes.len()
    );
    for window in retained_sizes.windows(2) {
        assert!(
            window[0] >= window[1],
            "objects not sorted by retained size descending: {} < {}",
            window[0],
            window[1]
        );
    }
}

#[test]
fn get_summary_expand_invalid_constructor() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "get_summary",
        serde_json::json!({ "snapshot_id": 1, "class_name": "NoSuchConstructor" }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("No constructor group"),
        "expected not-found error, got: {err}"
    );
}

#[test]
fn get_dominators_of() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "get_dominators_of",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@7165" }),
    );
    let text = get_text(&resp);
    assert!(
        text.contains("Dominator chain for @7165"),
        "expected dominator chain header, got: {text}"
    );
    assert!(
        text.contains("dominated by"),
        "expected at least one dominator, got: {text}"
    );
}

#[test]
fn multiple_snapshots_get_different_ids() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    let resp1 = proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": &path }));
    let resp2 = proc.call_tool(2, "load_snapshot", serde_json::json!({ "path": &path }));

    let text1 = get_text(&resp1);
    let text2 = get_text(&resp2);
    assert!(
        text1.contains("snapshot_id: 1"),
        "expected first id=1, got: {text1}"
    );
    assert!(
        text2.contains("snapshot_id: 2"),
        "expected second id=2, got: {text2}"
    );
}

// -----------------------------------------------------------------------
// compare_snapshots
// -----------------------------------------------------------------------
// compare_snapshots helpers
// -----------------------------------------------------------------------

/// Parse the overview table from compare_snapshots into structured rows.
struct CompDiffRow {
    name: String,
    new_count: i64,
    deleted_count: i64,
    delta_count: i64,
}

fn parse_compare_rows(text: &str) -> Vec<CompDiffRow> {
    let mut rows = Vec::new();
    for line in text.lines() {
        if line.contains("constructors with changes")
            || line.contains("Constructor")
            || line.trim().is_empty()
        {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }
        // Find where the name ends and numbers begin
        let mut name_end = 0;
        for (i, part) in parts.iter().enumerate() {
            let s = part.trim_start_matches('+').trim_start_matches('\u{2212}');
            if s.parse::<u64>().is_ok() {
                name_end = i;
                break;
            }
        }
        if name_end == 0 || name_end + 2 >= parts.len() {
            continue;
        }
        let name = parts[..name_end].join(" ");
        let parse_signed = |s: &str| -> i64 {
            s.replace('\u{2212}', "-")
                .replace('+', "")
                .replace(',', "")
                .parse::<i64>()
                .unwrap_or(0)
        };
        rows.push(CompDiffRow {
            name,
            new_count: parse_signed(parts[name_end]),
            deleted_count: parse_signed(parts[name_end + 1]),
            delta_count: parse_signed(parts[name_end + 2]),
        });
    }
    rows
}

fn find_comp_row<'a>(rows: &'a [CompDiffRow], name: &str) -> &'a CompDiffRow {
    rows.iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("no row found for '{name}'"))
}

// -----------------------------------------------------------------------
// compare_snapshots tests
// -----------------------------------------------------------------------

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
    let rows = parse_compare_rows(&text);

    let row = find_comp_row(&rows, "NewObject");
    assert_eq!(row.new_count, 2);
    assert_eq!(row.deleted_count, 0);
    assert_eq!(row.delta_count, 2);
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
    let rows = parse_compare_rows(&text);

    let row = find_comp_row(&rows, "InitialObject");
    assert_eq!(row.new_count, 0);
    assert_eq!(row.deleted_count, 2);
    assert_eq!(row.delta_count, -2);
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
    let rows = parse_compare_rows(&text);

    let new_obj = find_comp_row(&rows, "NewObject");
    assert_eq!(new_obj.new_count, 7);
    assert_eq!(new_obj.deleted_count, 0);
    assert_eq!(new_obj.delta_count, 7);

    let init_obj = find_comp_row(&rows, "InitialObject");
    assert_eq!(init_obj.new_count, 0);
    assert_eq!(init_obj.deleted_count, 2);
    assert_eq!(init_obj.delta_count, -2);
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
    let rows = parse_compare_rows(&text);
    assert!(
        rows.is_empty(),
        "identical snapshots should produce no diff rows"
    );
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
    assert!(
        text.contains("NewObject: # new: 2, # deleted: 0"),
        "expected NewObject header with counts, got: {text}"
    );
    // 2 new objects, each shown as "+ @..."
    let new_lines: Vec<_> = text.lines().filter(|l| l.contains("+ @")).collect();
    assert_eq!(
        new_lines.len(),
        2,
        "expected 2 new object lines, got: {new_lines:?}"
    );
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
    let object_lines: Vec<_> = text.lines().filter(|l| l.contains("+ @")).collect();
    assert_eq!(
        object_lines.len(),
        3,
        "expected 3 objects with limit=3, got: {object_lines:?}"
    );
    assert!(
        text.contains("Showing 1-3 of 7"),
        "expected paging status, got: {text}"
    );
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
    let rows = parse_compare_rows(&text);

    // Reversed: NewObject was in heap-2 (baseline), not heap-1 (main) → deleted
    let row = find_comp_row(&rows, "NewObject");
    assert_eq!(row.new_count, 0);
    assert_eq!(row.deleted_count, 2);
    assert_eq!(row.delta_count, -2);
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

// ── get_duplicate_strings ──────────────────────────────────────────────

#[test]
fn get_duplicate_strings_basic() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "get_duplicate_strings",
        serde_json::json!({ "snapshot_id": 1 }),
    );
    let text = get_text(&resp);
    assert!(
        text.contains("duplicate string groups"),
        "expected summary header, got: {text}"
    );
    assert!(
        text.contains("bytes wasted total"),
        "expected wasted bytes in header, got: {text}"
    );
}

#[test]
fn get_duplicate_strings_pagination() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    // Request only 2 entries
    let resp = proc.call_tool(
        2,
        "get_duplicate_strings",
        serde_json::json!({ "snapshot_id": 1, "limit": 2 }),
    );
    let text = get_text(&resp);
    assert!(
        text.contains("Showing entries 0..2"),
        "expected paginated range, got: {text}"
    );

    // Request with offset
    let resp2 = proc.call_tool(
        3,
        "get_duplicate_strings",
        serde_json::json!({ "snapshot_id": 1, "offset": 2, "limit": 2 }),
    );
    let text2 = get_text(&resp2);
    assert!(
        text2.contains("Showing entries 2..4"),
        "expected offset range, got: {text2}"
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
    assert!(
        text.contains("No closure leaks detected"),
        "expected no leaks, got: {text}"
    );
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
    assert!(
        text.contains("(incomplete:"),
        "expected incomplete markers with reason, got: {text}"
    );
    assert!(
        text.contains("contexts with unused variables"),
        "expected summary line, got: {text}"
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

// ── get_timeline ──────────────────────────────────────────────────────

#[test]
fn get_timeline_no_data() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(2, "get_timeline", serde_json::json!({ "snapshot_id": 1 }));
    let text = get_text(&resp);
    assert!(
        text.contains("No allocation timeline data") || text.contains("Allocation Timeline"),
        "expected timeline response, got: {text}"
    );
}

#[test]
fn get_timeline_invalid_snapshot() {
    let mut proc = McpProcess::start();

    let resp = proc.call_tool(1, "get_timeline", serde_json::json!({ "snapshot_id": 999 }));
    let err = get_error_message(&resp);
    assert!(
        err.contains("No snapshot found"),
        "expected not-found error, got: {err}"
    );
}
