use crate::snapshot::HeapSnapshot;

use super::format_size;

pub fn print_timeline(snap: &HeapSnapshot) {
    let intervals = snap.get_timeline();
    if intervals.is_empty() {
        println!("No allocation timeline data in this snapshot.");
        return;
    }

    let total_count: u64 = intervals.iter().map(|i| i.count as u64).sum();
    let total_size: u64 = intervals.iter().map(|i| i.size).sum();
    let max_size = intervals.iter().map(|i| i.size).max().unwrap_or(1).max(1);

    println!(
        "Allocation Timeline ({} intervals, {} live objects, {} total):",
        intervals.len(),
        total_count,
        format_size(total_size),
    );
    println!();

    let bar_width = 50;
    for interval in intervals {
        let ts_sec = interval.timestamp_us as f64 / 1_000_000.0;
        let w = ((interval.size as f64 / max_size as f64) * bar_width as f64).round() as usize;
        let bar = "\u{2588}".repeat(w);
        println!(
            "  {:>6.1}s  {:>8}  {:>5} obj  {}",
            ts_sec,
            format_size(interval.size),
            interval.count,
            bar,
        );
    }
}
