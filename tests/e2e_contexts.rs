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

fn run_contexts(file: &str, extra: &[&str]) -> String {
    let path = format!("{}/{}", test_dir(), file);
    let mut cmd = heap_snapshot_bin();
    cmd.arg("contexts").arg(&path);
    for arg in extra {
        cmd.arg(arg);
    }
    let output = cmd.output().expect("failed to run heap-snapshot");
    assert!(output.status.success(), "exit code: {}", output.status);
    String::from_utf8(output.stdout).expect("invalid utf-8")
}

#[test]
fn realms_heap1() {
    let output = run_realms("heap-1.heapsnapshot");
    assert_content!(output, "expected_realms_heap1.txt");
}

#[test]
fn contexts_heap1_minimum_retained_size_zero() {
    let output = run_contexts("heap-1.heapsnapshot", &["--minimum-retained-size", "0"]);
    assert_content!(output, "expected_contexts_heap1_min_retained_0.txt");
}

#[test]
fn contexts_heap1_default_minimum_retained_size() {
    let output = run_contexts("heap-1.heapsnapshot", &[]);
    assert_content!(output, "expected_contexts_heap1_default.txt");
}

#[test]
fn contexts_heap1_minimum_var_retained_size_3k() {
    let output = run_contexts(
        "heap-1.heapsnapshot",
        &[
            "--minimum-retained-size",
            "0",
            "--minimum-var-retained-size",
            "3K",
        ],
    );
    assert_content!(output, "expected_contexts_heap1_min_var_retained_3k.txt");
}

#[test]
fn contexts_closures_minimum_retained_size_zero() {
    let output = run_contexts("closures.heapsnapshot", &["--minimum-retained-size", "0"]);
    assert_content!(output, "expected_contexts_closures_min_retained_0.txt");
}

#[test]
fn contexts_closures_minimum_retained_size_100() {
    let output = run_contexts("closures.heapsnapshot", &["--minimum-retained-size", "100"]);
    assert_content!(output, "expected_contexts_closures_min_retained_100.txt");
}

#[test]
fn contexts_closures_minimum_var_retained_size_zero() {
    let output = run_contexts(
        "closures.heapsnapshot",
        &[
            "--minimum-retained-size",
            "0",
            "--minimum-var-retained-size",
            "0",
        ],
    );
    assert_content!(output, "expected_contexts_closures_min_var_retained_0.txt");
}
