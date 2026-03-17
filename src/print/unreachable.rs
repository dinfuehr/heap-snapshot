use crate::snapshot::HeapSnapshot;
use crate::types::Distance;

use super::{
    COL_DIST, COL_NAME_SUMMARY, COL_SHALLOW, COL_SHALLOW_PCT, format_count, format_distance,
    format_size, pct_str,
};

pub fn print_unreachable(snap: &HeapSnapshot, roots_only: bool) {
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

    let col_count: usize = 12;
    let total_width = COL_NAME_SUMMARY + col_count + COL_DIST + COL_SHALLOW + COL_SHALLOW_PCT;

    // When --full is set, recompute per-group stats for root-only nodes
    struct DisplayEntry {
        name: String,
        count: u32,
        distance: Distance,
        self_size: f64,
    }

    let display_entries: Vec<DisplayEntry> = entries
        .iter()
        .filter_map(|entry| {
            if !roots_only {
                return Some(DisplayEntry {
                    name: entry.name.clone(),
                    count: entry.count,
                    distance: entry.distance,
                    self_size: entry.self_size,
                });
            }
            let mut count = 0u32;
            let mut self_size = 0.0f64;
            let mut min_dist = Distance::NONE;
            for ord in &entry.node_ordinals {
                let d = snap.node_distance(*ord);
                if d.is_unreachable_root() {
                    count += 1;
                    self_size += snap.node_self_size(*ord) as f64;
                    min_dist = min_dist.min(d);
                }
            }
            if count == 0 {
                return None;
            }
            Some(DisplayEntry {
                name: entry.name.clone(),
                count,
                distance: min_dist,
                self_size,
            })
        })
        .collect();

    let total_size: f64 = display_entries.iter().map(|e| e.self_size).sum();
    let total_count: u32 = display_entries.iter().map(|e| e.count).sum();

    if total_count == 0 {
        println!("No fully unreachable objects found.");
        return;
    }

    println!(
        "{:<w_name$}{:>w_count$}{:>w_dist$}{:>w_ss$}",
        "Constructor",
        "Count",
        "Dist",
        "Shallow Size",
        w_name = COL_NAME_SUMMARY,
        w_count = col_count,
        w_dist = COL_DIST,
        w_ss = COL_SHALLOW + COL_SHALLOW_PCT,
    );
    println!("{}", "\u{2500}".repeat(total_width));

    for entry in &display_entries {
        let count_str = format!("\u{00d7}{}", format_count(entry.count));
        let name = format!("{} {count_str}", entry.name);
        let shallow_pct = pct_str(entry.self_size, total_size);
        println!(
            "{:<w_name$}{:>w_count$}{:>w_dist$}{:>w_s$}{:>w_sp$}",
            name,
            "",
            format_distance(entry.distance),
            format_size(entry.self_size),
            shallow_pct,
            w_name = COL_NAME_SUMMARY,
            w_count = col_count,
            w_dist = COL_DIST,
            w_s = COL_SHALLOW,
            w_sp = COL_SHALLOW_PCT,
        );
    }

    println!("{}", "\u{2500}".repeat(total_width));
    println!(
        "{:<w_name$}{:>w_count$}{:>w_dist$}{:>w_s$}{:>w_sp$}",
        format!(
            "Total ({} constructors, {} objects)",
            display_entries.len(),
            format_count(total_count)
        ),
        "",
        "",
        format_size(total_size),
        "100%",
        w_name = COL_NAME_SUMMARY,
        w_count = col_count,
        w_dist = COL_DIST,
        w_s = COL_SHALLOW,
        w_sp = COL_SHALLOW_PCT,
    );

    if roots_only {
        return;
    }

    // Distance breakdown
    let mut root_count = 0u32;
    let mut root_size = 0.0f64;
    let mut transitive_count = 0u32;
    let mut transitive_size = 0.0f64;
    for entry in &entries {
        for ord in &entry.node_ordinals {
            let d = snap.node_distance(*ord);
            let sz = snap.node_self_size(*ord) as f64;
            if d.is_unreachable_root() {
                root_count += 1;
                root_size += sz;
            } else {
                transitive_count += 1;
                transitive_size += sz;
            }
        }
    }

    println!();
    println!(
        "Distance breakdown: {} root (U, {}), {} transitive (U+N, {})",
        format_count(root_count),
        format_size(root_size),
        format_count(transitive_count),
        format_size(transitive_size),
    );

    // Show per-distance counts if there are transitive nodes
    if transitive_count > 0 {
        let mut dist_counts: std::collections::BTreeMap<u32, (u32, f64)> =
            std::collections::BTreeMap::new();
        for entry in &entries {
            for ord in &entry.node_ordinals {
                let d = snap.node_distance(*ord);
                let offset = d.0 - Distance::UNREACHABLE_BASE.0;
                let sz = snap.node_self_size(*ord) as f64;
                let e = dist_counts.entry(offset).or_insert((0, 0.0));
                e.0 += 1;
                e.1 += sz;
            }
        }
        for (offset, (count, size)) in &dist_counts {
            let label = if *offset == 0 {
                "U".to_string()
            } else {
                format!("U+{offset}")
            };
            println!(
                "  {:<6} {:>6} objects  {}",
                label,
                format_count(*count),
                format_size(*size),
            );
        }
    }
}
