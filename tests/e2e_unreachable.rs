mod common;
use common::{heap_snapshot_bin, test_dir};

fn run_unreachable(file: &str, extra: &[&str]) -> String {
    let path = format!("{}/{}", test_dir(), file);
    let mut cmd = heap_snapshot_bin();
    cmd.arg("summary").arg("--unreachable").arg(&path);
    for arg in extra {
        cmd.arg(arg);
    }
    let output = cmd.output().expect("failed to run heap-snapshot");
    assert!(output.status.success(), "exit code: {}", output.status);
    String::from_utf8(output.stdout).expect("invalid utf-8")
}

#[test]
fn unreachable_no_objects() {
    let output = run_unreachable("heap-1.heapsnapshot", &[]);
    assert!(
        output.contains("No unreachable objects found"),
        "expected no unreachable message"
    );
}
