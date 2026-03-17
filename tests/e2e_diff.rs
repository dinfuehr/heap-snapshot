mod common;
use common::{heap_snapshot_bin, test_dir};

fn run_diff(main: &str, compare: &str, extra: &[&str]) -> String {
    let main_path = format!("{}/{}", test_dir(), main);
    let compare_path = format!("{}/{}", test_dir(), compare);
    let mut cmd = heap_snapshot_bin();
    cmd.arg("diff").arg(&main_path).arg(&compare_path);
    for arg in extra {
        cmd.arg(arg);
    }
    let output = cmd.output().expect("failed to run heap-snapshot");
    assert!(output.status.success(), "exit code: {}", output.status);
    // stdout has "Reading..." lines on stderr via println, but actually
    // our tool uses println so it all goes to stdout. Filter the data lines.
    String::from_utf8(output.stdout).expect("invalid utf-8")
}

/// Find the line matching a constructor name and parse its columns.
struct DiffRow {
    name: String,
    new_count: i64,
    deleted_count: i64,
    delta_count: i64,
}

fn parse_diff_rows(output: &str) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    for line in output.lines() {
        // Skip header, separator, status lines
        if line.starts_with("Constructor")
            || line.starts_with('\u{2500}')
            || line.starts_with("Reading")
            || line.starts_with("Initializing")
            || line.starts_with("  ")
            || line.trim().is_empty()
        {
            continue;
        }
        // Lines look like: "▶ NewObject                 2           0          +2  ..."
        // Strip the marker
        let trimmed = line
            .trim_start_matches('\u{25b6}') // ▶
            .trim_start_matches('\u{25bc}') // ▼
            .trim();

        // Split into columns — name is everything up to the first number column
        // Numbers are right-aligned, so find the first token that looks numeric
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }

        // Walk from the end to find where numbers start
        // The last 6 tokens are: #New, #Deleted, #Delta, AllocSize, FreedSize, SizeDelta
        // but sizes have units (e.g. "120 B", "3 kB"), so count from the right.
        // Instead, find the constructor name by looking for the first token that
        // parses as a number or starts with + or −
        let mut name_end = 0;
        for (i, part) in parts.iter().enumerate() {
            let s = part.trim_start_matches('+').trim_start_matches('\u{2212}'); // −
            if s.parse::<u64>().is_ok() {
                name_end = i;
                break;
            }
        }
        if name_end == 0 {
            continue;
        }

        let name = parts[..name_end].join(" ");
        // parts[name_end] = #New, parts[name_end+1] = #Deleted, parts[name_end+2] = #Delta
        let parse_signed = |s: &str| -> i64 {
            s.replace('\u{2212}', "-")
                .replace('+', "")
                .replace(',', "")
                .parse::<i64>()
                .unwrap_or(0)
        };

        if name_end + 2 >= parts.len() {
            continue;
        }
        let new_count = parse_signed(parts[name_end]);
        let deleted_count = parse_signed(parts[name_end + 1]);
        let delta_count = parse_signed(parts[name_end + 2]);

        rows.push(DiffRow {
            name,
            new_count,
            deleted_count,
            delta_count,
        });
    }
    rows
}

fn find_row<'a>(rows: &'a [DiffRow], name: &str) -> &'a DiffRow {
    rows.iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("no row found for '{name}'"))
}

// ── Tests ────────────────────────────────────────────────────────────────

#[test]
fn diff_heap2_vs_heap1_new_object_delta() {
    let output = run_diff("heap-2.heapsnapshot", "heap-1.heapsnapshot", &[]);
    let rows = parse_diff_rows(&output);
    let row = find_row(&rows, "NewObject");
    assert_eq!(row.new_count, 2);
    assert_eq!(row.deleted_count, 0);
    assert_eq!(row.delta_count, 2);
}

#[test]
fn diff_heap2_vs_heap1_initial_object_deleted() {
    let output = run_diff("heap-2.heapsnapshot", "heap-1.heapsnapshot", &[]);
    let rows = parse_diff_rows(&output);
    let row = find_row(&rows, "InitialObject");
    assert_eq!(row.new_count, 0);
    assert_eq!(row.deleted_count, 2);
    assert_eq!(row.delta_count, -2);
}

#[test]
fn diff_heap3_vs_heap1_new_object_delta() {
    let output = run_diff("heap-3.heapsnapshot", "heap-1.heapsnapshot", &[]);
    let rows = parse_diff_rows(&output);
    let row = find_row(&rows, "NewObject");
    assert_eq!(row.new_count, 7);
    assert_eq!(row.deleted_count, 0);
    assert_eq!(row.delta_count, 7);
}

#[test]
fn diff_heap3_vs_heap1_initial_object_deleted() {
    let output = run_diff("heap-3.heapsnapshot", "heap-1.heapsnapshot", &[]);
    let rows = parse_diff_rows(&output);
    let row = find_row(&rows, "InitialObject");
    assert_eq!(row.new_count, 0);
    assert_eq!(row.deleted_count, 2);
    assert_eq!(row.delta_count, -2);
}

#[test]
fn diff_reversed_flips_direction() {
    let output = run_diff("heap-1.heapsnapshot", "heap-2.heapsnapshot", &[]);
    let rows = parse_diff_rows(&output);
    let row = find_row(&rows, "NewObject");
    assert_eq!(row.new_count, 0);
    assert_eq!(row.deleted_count, 2);
    assert_eq!(row.delta_count, -2);
}

#[test]
fn diff_identical_produces_no_rows() {
    let output = run_diff("heap-1.heapsnapshot", "heap-1.heapsnapshot", &[]);
    let rows = parse_diff_rows(&output);
    assert!(
        rows.is_empty(),
        "identical snapshots should produce no diff rows"
    );
}

#[test]
fn diff_expand_group_shows_members() {
    let output = run_diff(
        "heap-2.heapsnapshot",
        "heap-1.heapsnapshot",
        &["-g", "NewObject"],
    );
    // Expanded group should have child lines starting with "  ▶ NewObject @"
    let member_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("\u{25b6} NewObject @"))
        .collect();
    assert_eq!(
        member_lines.len(),
        2,
        "expected 2 expanded NewObject members"
    );
}

#[test]
fn diff_expand_group_with_window() {
    let output = run_diff(
        "heap-3.heapsnapshot",
        "heap-1.heapsnapshot",
        &["-g", "NewObject:0:3"],
    );
    let member_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("\u{25b6} NewObject @"))
        .collect();
    assert_eq!(member_lines.len(), 3, "expected 3 members with window :0:3");
    assert!(
        output.contains("of 7 members"),
        "expected 'of 7 members' status line"
    );
}
