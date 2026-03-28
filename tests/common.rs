use std::process::Command;

#[allow(dead_code)]
pub fn heap_snapshot_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_heap-snapshot"))
}

pub fn test_dir() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data")
}
