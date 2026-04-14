use rustc_hash::FxHashSet;

use super::{
    COL_DIST, COL_NAME_SUMMARY, COL_RETAINED, COL_RETAINED_PCT, COL_SHALLOW, COL_SHALLOW_PCT,
    ExpandMap, GroupExpandMap, display_width, format_count, format_distance, format_size, pad_str,
    pct_str, total_width, truncate_str,
};
use crate::snapshot::HeapSnapshot;
use crate::types::{Distance, NodeOrdinal};

pub enum SummaryFilter {
    All,
    Unreachable,
    UnreachableRoots,
    RetainedByDetachedDom,
    RetainedByConsole,
    RetainedByEventHandlers,
    /// Objects allocated in a specific timeline interval (by index).
    Interval(usize),
}

fn print_row(
    display: &str,
    dist: Distance,
    shallow: u64,
    retained: u64,
    total_shallow: u64,
    total_retained: u64,
) {
    let dist_str = format_distance(dist);
    let name_col = pad_str(display, COL_NAME_SUMMARY);
    println!(
        "{}{:>w_d$}{:>w_s$}{:>w_sp$}{:>w_r$}{:>w_rp$}",
        name_col,
        dist_str,
        format_size(shallow),
        pct_str(shallow, total_shallow),
        format_size(retained),
        pct_str(retained, total_retained),
        w_d = COL_DIST,
        w_s = COL_SHALLOW,
        w_sp = COL_SHALLOW_PCT,
        w_r = COL_RETAINED,
        w_rp = COL_RETAINED_PCT,
    );
}

fn print_row_shallow(display: &str, dist: Distance, shallow: u64, total_shallow: u64) {
    let dist_str = format_distance(dist);
    let name_col = pad_str(display, COL_NAME_SUMMARY);
    println!(
        "{}{:>w_d$}{:>w_s$}{:>w_sp$}",
        name_col,
        dist_str,
        format_size(shallow),
        pct_str(shallow, total_shallow),
        w_d = COL_DIST,
        w_s = COL_SHALLOW,
        w_sp = COL_SHALLOW_PCT,
    );
}

fn walk_edges(
    snap: &HeapSnapshot,
    node_ordinal: NodeOrdinal,
    depth: usize,
    max_depth: usize,
    base_indent: &str,
    expand: &ExpandMap,
    visited: &mut FxHashSet<NodeOrdinal>,
    is_filtered: bool,
    total_shallow: u64,
    total_retained: u64,
) {
    let w = expand
        .get(&snap.node_id(node_ordinal))
        .copied()
        .unwrap_or_default();
    let total_edges = snap
        .iter_edges(node_ordinal)
        .filter(|&(edge_idx, _)| !snap.is_invisible_edge(edge_idx))
        .count();
    let start = w.start.min(total_edges);
    let end = (start + w.count).min(total_edges);
    let shown = end - start;

    for (edge_idx, child_ordinal) in snap
        .iter_edges(node_ordinal)
        .filter(|&(edge_idx, _)| !snap.is_invisible_edge(edge_idx))
        .skip(start)
        .take(w.count)
    {
        let child_id = snap.node_id(child_ordinal);

        let has_children = snap.node_edge_count(child_ordinal) > 0;
        let should_expand = has_children
            && !visited.contains(&child_ordinal)
            && (depth < max_depth || expand.contains_key(&child_id));
        let marker = if should_expand {
            "\u{25bc}" /* ▼ */
        } else {
            "\u{25b6}" /* ▶ */
        };
        let edge_label = snap.format_edge_label(edge_idx, child_ordinal);
        let label = format!("{base_indent}{marker} {edge_label}");
        let display = truncate_str(&label, COL_NAME_SUMMARY);

        if is_filtered {
            print_row_shallow(
                &display,
                snap.node_distance(child_ordinal),
                snap.node_self_size(child_ordinal) as u64,
                total_shallow,
            );
        } else {
            print_row(
                &display,
                snap.node_distance(child_ordinal),
                snap.node_self_size(child_ordinal) as u64,
                snap.node_retained_size(child_ordinal),
                total_shallow,
                total_retained,
            );
        }

        if should_expand {
            visited.insert(child_ordinal);
            let child_indent = format!("{base_indent}  ");
            walk_edges(
                snap,
                child_ordinal,
                depth + 1,
                max_depth,
                &child_indent,
                expand,
                visited,
                is_filtered,
                total_shallow,
                total_retained,
            );
            visited.remove(&child_ordinal);
        }
    }
    if shown < total_edges {
        println!(
            // \u{2013} = –
            "{base_indent}  {}\u{2013}{} of {total_edges} refs",
            start + 1,
            start + shown
        );
    }
}

pub fn print_summary(
    snap: &HeapSnapshot,
    max_depth: usize,
    expand_constructors: &GroupExpandMap,
    expand_ids: &ExpandMap,
    filter: SummaryFilter,
) {
    let is_filtered = !matches!(filter, SummaryFilter::All);

    println!("Computing aggregates...");
    let aggregates = match filter {
        SummaryFilter::All => snap.aggregates_with_filter(),
        SummaryFilter::Unreachable => snap.unreachable_aggregates(),
        SummaryFilter::UnreachableRoots => snap.unreachable_root_aggregates(),
        SummaryFilter::RetainedByDetachedDom => snap.retained_by_detached_dom(),
        SummaryFilter::RetainedByConsole => snap.retained_by_console(),
        SummaryFilter::RetainedByEventHandlers => snap.retained_by_event_handlers(),
        SummaryFilter::Interval(idx) => {
            let intervals = snap.get_timeline();
            if idx >= intervals.len() {
                println!(
                    "Invalid interval index {idx}, snapshot has {} intervals.",
                    intervals.len()
                );
                return;
            }
            let interval = &intervals[idx];
            let ts_sec = interval.timestamp_us as f64 / 1_000_000.0;
            println!(
                "Interval {} ({:.1}s): {} objects, {}",
                idx,
                ts_sec,
                interval.count,
                format_size(interval.size),
            );
            snap.aggregates_for_id_range(interval.id_from, interval.id_to)
        }
    };

    if is_filtered && aggregates.is_empty() {
        println!("No matching objects found.");
        return;
    }

    let mut entries: Vec<_> = aggregates.iter().collect();
    if is_filtered {
        entries.sort_by(|a, b| {
            b.self_size
                .cmp(&a.self_size)
                .then(a.first_seen.cmp(&b.first_seen))
        });
    } else {
        entries.sort_by(|a, b| {
            b.max_ret
                .cmp(&a.max_ret)
                .then(a.first_seen.cmp(&b.first_seen))
        });
    }

    let total_shallow: u64 = entries.iter().map(|e| e.self_size).sum();
    let total_retained: u64 = entries.iter().map(|e| e.max_ret).sum();

    let tw = if is_filtered {
        COL_NAME_SUMMARY + COL_DIST + COL_SHALLOW + COL_SHALLOW_PCT
    } else {
        total_width(COL_NAME_SUMMARY)
    };

    if is_filtered {
        println!(
            "{:<w_name$}{:>w_dist$}{:>w_ss$}",
            "Constructor",
            "Distance",
            "Shallow Size",
            w_name = COL_NAME_SUMMARY,
            w_dist = COL_DIST,
            w_ss = COL_SHALLOW + COL_SHALLOW_PCT,
        );
    } else {
        println!(
            "{:<w_name$}{:>w_dist$}{:>w_ss$}{:>w_rs$}",
            "Constructor",
            "Distance",
            "Shallow Size",
            "Retained Size",
            w_name = COL_NAME_SUMMARY,
            w_dist = COL_DIST,
            w_ss = COL_SHALLOW + COL_SHALLOW_PCT,
            w_rs = COL_RETAINED + COL_RETAINED_PCT,
        );
    }
    println!(
        "{}",
        "\u{2500}" /* ─ */
            .repeat(tw)
    );

    for entry in &entries {
        // Constructor is expanded if explicitly requested or if any --expand ID matches a node in it
        let has_expanded_node = !expand_ids.is_empty()
            && entry
                .node_ordinals
                .iter()
                .any(|&o| expand_ids.contains_key(&snap.node_id(o)));
        let group_window = expand_constructors.get(&entry.name).or_else(|| {
            // Match by base name (without location bracket suffix).
            let base = entry.name.split(" [").next().unwrap_or(&entry.name);
            expand_constructors.get(base)
        });
        let is_expanded = group_window.is_some() || has_expanded_node;
        let marker = if is_expanded {
            "\u{25bc} " /* ▼ */
        } else {
            "\u{25b6} " /* ▶ */
        };
        let count_str = format!("\u{00d7}{}" /* × */, format_count(entry.count));
        let max_name_len =
            COL_NAME_SUMMARY.saturating_sub(display_width(&count_str) + display_width(marker) + 3);
        let display_name = truncate_str(&entry.name, max_name_len);
        let name_col = pad_str(
            &format!("{marker}{}  {}", display_name, count_str),
            COL_NAME_SUMMARY,
        );

        if is_filtered {
            print_row_shallow(&name_col, entry.distance, entry.self_size, total_shallow);
        } else {
            print_row(
                &name_col,
                entry.distance,
                entry.self_size,
                entry.max_ret,
                total_shallow,
                total_retained,
            );
        }

        if is_expanded {
            let total_members = entry.node_ordinals.len();
            let w = group_window.copied().unwrap_or_default();
            let start = w.start.min(total_members);
            let end = (start + w.count).min(total_members);
            let members = &entry.node_ordinals[start..end];

            for &ordinal in members {
                let id = snap.node_id(ordinal);
                let node_expanded = expand_ids.contains_key(&id);
                let node_marker = if node_expanded {
                    "\u{25bc}" /* ▼ */
                } else {
                    "\u{25b6}" /* ▶ */
                };
                let label = format!("  {node_marker} {} @{id}", entry.name);
                let display = truncate_str(&label, COL_NAME_SUMMARY);

                if is_filtered {
                    print_row_shallow(
                        &display,
                        snap.node_distance(ordinal),
                        snap.node_self_size(ordinal) as u64,
                        total_shallow,
                    );
                } else {
                    print_row(
                        &display,
                        snap.node_distance(ordinal),
                        snap.node_self_size(ordinal) as u64,
                        snap.node_retained_size(ordinal),
                        total_shallow,
                        total_retained,
                    );
                }

                if node_expanded {
                    let mut visited: FxHashSet<NodeOrdinal> = FxHashSet::default();
                    visited.insert(ordinal);
                    walk_edges(
                        snap,
                        ordinal,
                        0,
                        max_depth,
                        "    ",
                        expand_ids,
                        &mut visited,
                        is_filtered,
                        total_shallow,
                        total_retained,
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

    println!(
        "{}",
        "\u{2500}" /* ─ */
            .repeat(tw)
    );
    if is_filtered {
        println!(
            "{:<w_name$}{:>w_dist$}{:>w_s$}{:>w_sp$}",
            format!("Total ({} constructors)", entries.len()),
            "",
            format_size(total_shallow),
            "100%",
            w_name = COL_NAME_SUMMARY,
            w_dist = COL_DIST,
            w_s = COL_SHALLOW,
            w_sp = COL_SHALLOW_PCT,
        );
    } else {
        println!(
            "{:<w_name$}{:>w_dist$}{:>w_s$}{:>w_sp$}{:>w_r$}{:>w_rp$}",
            format!("Total ({} constructors)", entries.len()),
            "",
            format_size(total_shallow),
            "100%",
            format_size(total_retained),
            "100%",
            w_name = COL_NAME_SUMMARY,
            w_dist = COL_DIST,
            w_s = COL_SHALLOW,
            w_sp = COL_SHALLOW_PCT,
            w_r = COL_RETAINED,
            w_rp = COL_RETAINED_PCT,
        );
    }
}
