use std::process::Command;

#[allow(dead_code)]
pub fn heap_snapshot_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_heap-snapshot"))
}

pub fn test_dir() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data")
}

/// Assert that `$value`, with leading "Reading …" / "Initializing …"
/// progress lines stripped, equals the contents of `tests/data/$filename`.
/// `$filename` must be a string literal.
#[macro_export]
macro_rules! assert_content {
    ($value:expr, $filename:literal) => {{
        let expected = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/",
            $filename
        ));
        let actual: String = ($value)
            .lines()
            .skip_while(|l| {
                l.is_empty() || l.starts_with("Reading") || l.starts_with("Initializing")
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert_eq!(actual, expected, "content mismatch against {}", $filename);
    }};
}
