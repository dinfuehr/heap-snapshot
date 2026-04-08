use crate::snapshot::HeapSnapshot;

use super::{format_count, format_size};

pub fn print_duplicate_strings(snap: &HeapSnapshot, min_count: u32) {
    let duplicates: Vec<_> = snap
        .duplicate_strings()
        .into_iter()
        .filter(|d| d.count >= min_count)
        .collect();

    if duplicates.is_empty() {
        println!("No duplicate strings found.");
        return;
    }

    let total_wasted: f64 = duplicates.iter().map(|d| d.wasted_size()).sum();
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
        let preview = if d.value.len() > col_value {
            let mut end = col_value - 1;
            while end > 0 && !d.value.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}\u{2026}", &d.value[..end])
        } else {
            d.value.clone()
        };
        let preview = preview.replace('\n', "\\n").replace('\r', "\\r");
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
}
