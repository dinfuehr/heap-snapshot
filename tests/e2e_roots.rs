mod common;
use common::{heap_snapshot_bin, test_dir};

fn run_roots(file: &str) -> String {
    let path = format!("{}/{}", test_dir(), file);
    let output = heap_snapshot_bin()
        .arg("roots")
        .arg(&path)
        .output()
        .expect("failed to run heap-snapshot");
    assert!(output.status.success(), "exit code: {}", output.status);
    String::from_utf8(output.stdout).expect("invalid utf-8")
}

#[test]
fn roots_heap1() {
    let output = run_roots("heap-1.heapsnapshot");
    assert_content!(output, "expected_roots_heap1.txt");
}
