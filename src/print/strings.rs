use crate::snapshot::HeapSnapshot;
use crate::display::truncate_str;

use super::{format_count, format_size};

pub fn print_duplicate_strings(
    snap: &HeapSnapshot,
    min_count: u32,
    offset: usize,
    limit: usize,
    show_object_ids: bool,
) {
    let result = snap.duplicate_strings();
    let duplicates: Vec<_> = result
        .duplicates
        .into_iter()
        .filter(|d| d.count >= min_count)
        .collect();

    if duplicates.is_empty() {
        println!("No duplicate strings found.");
        if result.skipped_count > 0 {
            println!(
                "({} strings ({}) skipped — no length metadata, use a newer V8 snapshot)",
                format_count(result.skipped_count),
                format_size(result.skipped_size),
            );
        }
        return;
    }

    let total_wasted: u64 = duplicates.iter().map(|d| d.wasted_size()).sum();
    let total = duplicates.len();
    let start = offset.min(total);
    let end = (start + limit).min(total);
    let col_count: usize = 8;
    let col_size: usize = 12;
    let col_wasted: usize = 12;
    let col_value: usize = 60;
    let total_width = col_count + col_size + col_wasted + col_value;

    if start >= end {
        println!(
            "Offset {offset} is past the end of the list ({total} duplicate groups available)."
        );
        if result.skipped_count > 0 {
            println!(
                "({} strings ({}) skipped — no length metadata, use a newer V8 snapshot)",
                format_count(result.skipped_count),
                format_size(result.skipped_size),
            );
        }
        return;
    }

    println!(
        "{:>w_count$}{:>w_size$}{:>w_wasted$}  {:<w_value$}",
        "Count",
        "Size",
        "Wasted",
        "Value",
        w_count = col_count,
        w_size = col_size,
        w_wasted = col_wasted,
        w_value = col_value,
    );
    println!("{}", "\u{2500}".repeat(total_width + 2));

    for d in &duplicates[start..end] {
        let mut preview = truncate_str(&d.value, col_value)
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        if d.truncated {
            preview = format!("{preview}\u{2026} (len {})", d.length);
        }
        println!(
            "{:>w_count$}{:>w_size$}{:>w_wasted$}  {}",
            format!("\u{00d7}{}", format_count(d.count)),
            format_size(d.total_size),
            format_size(d.wasted_size()),
            preview,
            w_count = col_count,
            w_size = col_size,
            w_wasted = col_wasted,
        );
        if show_object_ids {
            let ids = d
                .node_ids
                .iter()
                .map(|id| format!("@{}", id.0))
                .collect::<Vec<_>>()
                .join(", ");
            println!(
                "{:>w_prefix$}  {}",
                "",
                ids,
                w_prefix = col_count + col_size + col_wasted,
            );
        }
    }

    println!("{}", "\u{2500}".repeat(total_width + 2));
    println!(
        "Showing {}-{} of {total} duplicate groups ({} wasted total)",
        start + 1,
        end,
        format_size(total_wasted),
    );
    if result.skipped_count > 0 {
        println!(
            "({} strings ({}) skipped — no length metadata, use a newer V8 snapshot)",
            format_count(result.skipped_count),
            format_size(result.skipped_size),
        );
    }
}
