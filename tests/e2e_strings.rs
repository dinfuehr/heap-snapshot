mod common;
use common::{heap_snapshot_bin, test_dir};

fn run_strings(file: &str, extra: &[&str]) -> String {
    let path = format!("{}/{}", test_dir(), file);
    let mut cmd = heap_snapshot_bin();
    cmd.arg("strings").arg(&path);
    for arg in extra {
        cmd.arg(arg);
    }
    let output = cmd.output().expect("failed to run heap-snapshot");
    assert!(output.status.success(), "exit code: {}", output.status);
    String::from_utf8(output.stdout).expect("invalid utf-8")
}

#[test]
fn strings_prints_showing_header() {
    let output = run_strings("heap-3.heapsnapshot", &["--limit", "10"]);
    assert_content!(output, "expected_strings_heap3.txt");
}

#[test]
fn strings_offset_limit_slices() {
    let first = run_strings("heap-3.heapsnapshot", &["--limit", "5"]);
    assert_content!(first, "expected_strings_heap3_limit5.txt");

    let offset = run_strings("heap-3.heapsnapshot", &["--offset", "2", "--limit", "5"]);
    assert_content!(offset, "expected_strings_heap3_offset2_limit5.txt");
}

#[test]
fn strings_offset_past_end() {
    let output = run_strings("heap-3.heapsnapshot", &["--offset", "99999"]);
    assert_content!(output, "expected_strings_heap3_offset_past_end.txt");
}

#[test]
fn strings_show_object_ids_flag() {
    let output = run_strings(
        "heap-3.heapsnapshot",
        &["--show-object-ids", "--limit", "3"],
    );
    assert_content!(output, "expected_strings_heap3_show_ids.txt");
}

#[test]
fn strings_no_object_ids_by_default() {
    let output = run_strings("heap-3.heapsnapshot", &["--limit", "3"]);
    assert_content!(output, "expected_strings_heap3_limit3.txt");
}
