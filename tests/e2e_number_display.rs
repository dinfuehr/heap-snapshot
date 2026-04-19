mod common;
use common::{heap_snapshot_bin, test_dir};

// primitives.heapsnapshot node ids (from primitives.js `obj`):
//   @26629 — `obj` ({t, f, n, u, i, d, s, nested})
//   @25551 — smi 42 (obj.i)
//   @27893 — double 12.75 (obj.d)

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
    let output = run(
        "containment",
        "primitives.heapsnapshot",
        &["@25551", "--depth", "0"],
    );
    assert!(
        output.contains("smi 42"),
        "expected 'smi 42' in output, got:\n{output}"
    );
}

#[test]
fn containment_heap_number_display_name() {
    let output = run(
        "containment",
        "primitives.heapsnapshot",
        &["@27893", "--depth", "0"],
    );
    assert!(
        output.contains("double 12.75"),
        "expected 'double 12.75' in output, got:\n{output}"
    );
}

#[test]
fn summary_smi_display_name() {
    let output = run("summary", "primitives.heapsnapshot", &["-e", "@26629"]);
    assert!(
        output.contains("smi 42"),
        "expected 'smi 42' in output, got:\n{output}"
    );
}

#[test]
fn summary_heap_number_display_name() {
    let output = run("summary", "primitives.heapsnapshot", &["-e", "@26629"]);
    assert!(
        output.contains("double 12.75"),
        "expected 'double 12.75' in output, got:\n{output}"
    );
}
