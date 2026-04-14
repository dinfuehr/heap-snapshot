#[path = "../common.rs"]
mod common;

mod compare_snapshots;
mod get_closure_leaks;
mod get_containment;
mod get_dominators_of;
mod get_duplicate_strings;
mod get_native_contexts;
mod get_reachable_size;
mod get_retaining_paths;
mod get_statistics;
mod get_summary;
mod get_timeline;
mod load_snapshot;
mod show;
mod show_retainers;

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

fn load_heap1(proc: &mut McpProcess) {
    let path = format!("{}/heap-1.heapsnapshot", test_dir());
    proc.call_tool(1, "load_snapshot", serde_json::json!({ "path": path }));
}
