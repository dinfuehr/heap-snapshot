mod common;
use common::{heap_snapshot_bin, test_dir};

fn run_stack(file: &str) -> String {
    let path = format!("{}/{}", test_dir(), file);
    let output = heap_snapshot_bin()
        .arg("stack")
        .arg(&path)
        .output()
        .expect("failed to run heap-snapshot");
    assert!(output.status.success(), "exit code: {}", output.status);
    String::from_utf8(output.stdout).expect("invalid utf-8")
}

#[test]
fn stack_shows_header() {
    let output = run_stack("heap-1.heapsnapshot");
    assert!(
        output.contains("Object")
            && output.contains("Retained Size")
            && output.contains("Reachable Size"),
        "expected header in output, got:\n{output}"
    );
}

#[test]
fn stack_finds_stack_rooted_objects() {
    let output = run_stack("heap-1.heapsnapshot");
    assert!(
        output.contains("(Stack roots)"),
        "expected (Stack roots) source in output, got:\n{output}"
    );
}

#[test]
fn stack_shows_count() {
    let output = run_stack("heap-1.heapsnapshot");
    assert!(
        output.contains("21 stack-rooted objects"),
        "expected 21 stack-rooted objects, got:\n{output}"
    );
}

#[test]
fn stack_contains_known_objects() {
    let output = run_stack("heap-1.heapsnapshot");
    assert!(
        output.contains("InitialObject @7171"),
        "expected InitialObject in output, got:\n{output}"
    );
    assert!(
        output.contains("keep @7169"),
        "expected keep in output, got:\n{output}"
    );
}

#[test]
fn stack_shows_reached_native_contexts() {
    let output = run_stack("heap-1.heapsnapshot");
    assert!(
        output.contains("\u{2192} [utility] @7165"),
        "expected arrow with NativeContext in output, got:\n{output}"
    );
}
