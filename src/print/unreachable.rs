use crate::snapshot::HeapSnapshot;

use super::{COL_NAME_SUMMARY, COL_SHALLOW, COL_SHALLOW_PCT, format_count, format_size, pct_str};

pub fn print_unreachable(snap: &HeapSnapshot) {
    let stats = snap.get_statistics();
    if stats.unreachable_count == 0 {
        println!("No unreachable objects found.");
        return;
    }

    println!("Computing unreachable aggregates...");
    let aggregates = snap.unreachable_aggregates();

    let mut entries: Vec<_> = aggregates.values().collect();
    entries.sort_by(|a, b| {
        b.self_size
            .partial_cmp(&a.self_size)
            .unwrap()
            .then(a.first_seen.cmp(&b.first_seen))
    });

    let total_size: f64 = entries.iter().map(|e| e.self_size).sum();
    let total_count: u32 = entries.iter().map(|e| e.count).sum();

    let col_count: usize = 12;
    println!(
        "{:<w_name$}{:>w_count$}{:>w_ss$}",
        "Constructor",
        "Count",
        "Shallow Size",
        w_name = COL_NAME_SUMMARY,
        w_count = col_count,
        w_ss = COL_SHALLOW + COL_SHALLOW_PCT,
    );
    println!(
        "{}",
        "\u{2500}".repeat(COL_NAME_SUMMARY + col_count + COL_SHALLOW + COL_SHALLOW_PCT)
    );

    for entry in &entries {
        let count_str = format!("\u{00d7}{}", format_count(entry.count));
        let name = format!("{} {count_str}", entry.name);
        let shallow_pct = pct_str(entry.self_size, total_size);
        println!(
            "{:<w_name$}{:>w_count$}{:>w_s$}{:>w_sp$}",
            name,
            "",
            format_size(entry.self_size),
            shallow_pct,
            w_name = COL_NAME_SUMMARY,
            w_count = col_count,
            w_s = COL_SHALLOW,
            w_sp = COL_SHALLOW_PCT,
        );
    }

    println!(
        "{}",
        "\u{2500}".repeat(COL_NAME_SUMMARY + col_count + COL_SHALLOW + COL_SHALLOW_PCT)
    );
    println!(
        "{:<w_name$}{:>w_count$}{:>w_s$}{:>w_sp$}",
        format!(
            "Total ({} constructors, {} objects)",
            entries.len(),
            format_count(total_count)
        ),
        "",
        format_size(total_size),
        "100%",
        w_name = COL_NAME_SUMMARY,
        w_count = col_count,
        w_s = COL_SHALLOW,
        w_sp = COL_SHALLOW_PCT,
    );
}
