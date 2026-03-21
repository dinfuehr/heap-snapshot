mod common;
use common::{heap_snapshot_bin, test_dir};

fn run(subcommand: &str, file: &str, extra: &[&str]) -> String {
    let path = format!("{}/{}", test_dir(), file);
    let mut cmd = heap_snapshot_bin();
    cmd.arg(subcommand).arg(&path);
    for arg in extra {
        cmd.arg(arg);
    }
    let output = cmd.output().expect("failed to run heap-snapshot");
    assert!(output.status.success(), "exit code: {}", output.status);
    String::from_utf8(output.stdout).expect("invalid utf-8")
}

#[test]
fn containment_smi_display_name() {
    let output = run("containment", "primitives.heapsnapshot", &["@19393", "--depth", "0"]);
    assert!(
        output.contains("smi 42"),
        "expected 'smi 42' in output, got:\n{output}"
    );
}

#[test]
fn containment_heap_number_display_name() {
    let output = run("containment", "primitives.heapsnapshot", &["@21089", "--depth", "0"]);
    assert!(
        output.contains("double 12.75"),
        "expected 'double 12.75' in output, got:\n{output}"
    );
}

#[test]
fn summary_smi_display_name() {
    let output = run("summary", "primitives.heapsnapshot", &["-e", "@20165"]);
    assert!(
        output.contains("smi 42"),
        "expected 'smi 42' in output, got:\n{output}"
    );
}

#[test]
fn summary_heap_number_display_name() {
    let output = run("summary", "primitives.heapsnapshot", &["-e", "@20165"]);
    assert!(
        output.contains("double 12.75"),
        "expected 'double 12.75' in output, got:\n{output}"
    );
}
