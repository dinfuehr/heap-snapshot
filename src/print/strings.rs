use crate::snapshot::HeapSnapshot;
use crate::utils::truncate_str;

use super::{format_count, format_size};

pub fn print_duplicate_strings(snap: &HeapSnapshot, min_count: u32) {
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
    let col_count: usize = 8;
    let col_size: usize = 12;
    let col_wasted: usize = 12;
    let col_value: usize = 60;
    let total_width = col_count + col_size + col_wasted + col_value;

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

    for d in &duplicates {
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
    }

    println!("{}", "\u{2500}".repeat(total_width + 2));
    println!(
        "{:>w_count$}{:>w_size$}{:>w_wasted$}  {} duplicate groups",
        "",
        "",
        format_size(total_wasted),
        duplicates.len(),
        w_count = col_count,
        w_size = col_size,
        w_wasted = col_wasted,
    );
    if result.skipped_count > 0 {
        println!(
            "({} strings ({}) skipped — no length metadata, use a newer V8 snapshot)",
            format_count(result.skipped_count),
            format_size(result.skipped_size),
        );
    }
}
