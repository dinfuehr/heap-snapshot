mod common;
use common::{heap_snapshot_bin, test_dir};

fn run_realms(file: &str) -> String {
    let path = format!("{}/{}", test_dir(), file);
    let output = heap_snapshot_bin()
        .arg("realms")
        .arg(&path)
        .output()
        .expect("failed to run heap-snapshot");
    assert!(output.status.success(), "exit code: {}", output.status);
    String::from_utf8(output.stdout).expect("invalid utf-8")
}

#[test]
fn realms_finds_native_context() {
    let output = run_realms("heap-1.heapsnapshot");
    assert!(
        output.contains("[utility] #0"),
        "expected native context in output, got:\n{output}"
    );
}

#[test]
fn realms_shows_all_columns() {
    let output = run_realms("heap-1.heapsnapshot");
    assert!(
        output.contains("Realm")
            && output.contains("Det")
            && output.contains("Shallow Size")
            && output.contains("Retained Size")
            && output.contains("Reachable Size"),
        "expected all column headers, got:\n{output}"
    );
}

#[test]
fn realms_formats_sizes() {
    let output = run_realms("heap-1.heapsnapshot");
    // Sizes should be formatted with units (kB, MB, B), not raw numbers
    assert!(
        output.contains("kB") || output.contains("MB") || output.contains(" B"),
        "expected formatted sizes, got:\n{output}"
    );
}
