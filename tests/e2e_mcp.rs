mod common;

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};

fn test_dir() -> &'static str {
    common::test_dir()
}

fn mcp_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_heap-snapshot-mcp"))
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
fn get_outgoing_references() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "get_outgoing_references",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@1" }),
    );
    let text = get_text(&resp);
    assert!(
        text.contains("Object @1"),
        "expected object header, got: {text}"
    );
    assert!(
        text.contains("outgoing references") || text.contains("no outgoing references"),
        "expected references section, got: {text}"
    );
}

#[test]
fn get_outgoing_references_invalid_object() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "get_outgoing_references",
        serde_json::json!({ "snapshot_id": 1, "object_id": "@999999999" }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("No object found"),
        "expected not-found error, got: {err}"
    );
}

#[test]
fn get_outgoing_references_invalid_format() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "get_outgoing_references",
        serde_json::json!({ "snapshot_id": 1, "object_id": "not_a_number" }),
    );
    let err = get_error_message(&resp);
    assert!(
        err.contains("Invalid object id"),
        "expected invalid id error, got: {err}"
    );
}

#[test]
fn get_outgoing_references_without_at_prefix() {
    let mut proc = McpProcess::start();
    let path = format!("{}/heap-1.heapsnapshot", test_dir());

    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));

    let resp = proc.call_tool(
        2,
        "get_outgoing_references",
        serde_json::json!({ "snapshot_id": 1, "object_id": "1" }),
    );
    let text = get_text(&resp);
    assert!(
        text.contains("Object @1"),
        "expected object_id without @ prefix to work, got: {text}"
    );
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
