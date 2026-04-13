pub use crate::diff::{ClassDiff, compute_diff};

use super::{GroupExpandMap, display_width, format_count, format_size, pad_str, truncate_str};
use crate::snapshot::HeapSnapshot;

const COL_DIFF_NAME: usize = 40;
const COL_DIFF_NUM: usize = 12;
const COL_DIFF_SIZE: usize = 14;

fn diff_total_width() -> usize {
    COL_DIFF_NAME + COL_DIFF_NUM * 3 + COL_DIFF_SIZE * 3
}

pub fn format_signed_count(n: i64) -> String {
    if n > 0 {
        format!("+{}", format_count(n as u32))
    } else if n < 0 {
        format!("\u{2212}{}" /* − */, format_count((-n) as u32))
    } else {
        "0".to_string()
    }
}

pub fn format_signed_size(bytes: f64) -> String {
    if bytes > 0.0 {
        format!("+{}", format_size(bytes))
    } else if bytes < 0.0 {
        format!("\u{2212}{}" /* − */, format_size(-bytes))
    } else {
        "0 B".to_string()
    }
}

pub fn print_diff(snap1: &HeapSnapshot, snap2: &HeapSnapshot, expand_groups: &GroupExpandMap) {
    let entries = compute_diff(snap1, snap2);

    // Print header
    println!(
        "{:<w_name$}{:>w_n$}{:>w_n$}{:>w_n$}{:>w_s$}{:>w_s$}{:>w_s$}",
        "Constructor",
        "# New",
        "# Deleted",
        "# Delta",
        "Alloc. Size",
        "Freed Size",
        "Size Delta",
        w_name = COL_DIFF_NAME,
        w_n = COL_DIFF_NUM,
        w_s = COL_DIFF_SIZE,
    );
    println!(
        "{}",
        "\u{2500}" /* ─ */
            .repeat(diff_total_width())
    );

    for diff in &entries {
        let group_window = expand_groups.get(&diff.name).or_else(|| {
            let base = diff.name.split(" [").next().unwrap_or(&diff.name);
            expand_groups.get(base)
        });
        let expand = group_window.is_some();
        let marker = if expand && (!diff.new_objects.is_empty() || !diff.deleted_objects.is_empty())
        {
            "\u{25bc} " /* ▼ */
        } else {
            "\u{25b6} " /* ▶ */
        };
        let max_name_len = COL_DIFF_NAME.saturating_sub(display_width(marker) + 1);
        let display_name = format!("{marker}{}", truncate_str(&diff.name, max_name_len));
        let name_col = pad_str(&display_name, COL_DIFF_NAME);
        println!(
            "{}{:>w_n$}{:>w_n$}{:>w_n$}{:>w_s$}{:>w_s$}{:>w_s$}",
            name_col,
            format_count(diff.new_count),
            format_count(diff.deleted_count),
            format_signed_count(diff.delta_count()),
            format_size(diff.alloc_size),
            format_size(diff.freed_size),
            format_signed_size(diff.size_delta()),
            w_n = COL_DIFF_NUM,
            w_s = COL_DIFF_SIZE,
        );

        if !expand {
            continue;
        }

        // Combine new + deleted, then apply window
        let all_members: Vec<(bool, &crate::types::NodeId, &u32)> = diff
            .new_objects
            .iter()
            .map(|(id, sz)| (true, id, sz))
            .chain(diff.deleted_objects.iter().map(|(id, sz)| (false, id, sz)))
            .collect();
        let total_members = all_members.len();
        let w = group_window.copied().unwrap_or_default();
        let start = w.start.min(total_members);
        let end = (start + w.count).min(total_members);

        for &(is_new, node_id, self_size) in &all_members[start..end] {
            if is_new {
                let label = format!("  \u{25b6} {} @{node_id}" /* ▶ */, diff.name);
                let display = pad_str(&truncate_str(&label, COL_DIFF_NAME), COL_DIFF_NAME);
                println!(
                    "{}{:>w_n$}{:>w_n$}{:>w_n$}{:>w_s$}{:>w_s$}{:>w_s$}",
                    display,
                    "\u{2022}", /* • */
                    "",
                    "",
                    format_size(*self_size as f64),
                    "",
                    "",
                    w_n = COL_DIFF_NUM,
                    w_s = COL_DIFF_SIZE,
                );
            } else {
                let label = format!("  \u{25b6} {} @{node_id}" /* ▶ */, diff.name);
                let display = pad_str(&truncate_str(&label, COL_DIFF_NAME), COL_DIFF_NAME);
                println!(
                    "{}{:>w_n$}{:>w_n$}{:>w_n$}{:>w_s$}{:>w_s$}{:>w_s$}",
                    display,
                    "",
                    "\u{2022}", /* • */
                    "",
                    "",
                    format_size(*self_size as f64),
                    "",
                    w_n = COL_DIFF_NUM,
                    w_s = COL_DIFF_SIZE,
                );
            }
        }

        if end < total_members || start > 0 {
            println!(
                "  {}\u{2013}{} of {} members",
                start + 1,
                end,
                total_members
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_signed_count() {
        assert_eq!(format_signed_count(5), "+5");
        assert_eq!(format_signed_count(-3), "\u{2212}3");
        assert_eq!(format_signed_count(0), "0");
    }

    #[test]
    fn test_format_signed_size() {
        assert_eq!(format_signed_size(100.0), "+100 B");
        assert_eq!(format_signed_size(-200.0), "\u{2212}200 B");
        assert_eq!(format_signed_size(0.0), "0 B");
    }
}
