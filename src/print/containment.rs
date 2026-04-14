use rustc_hash::FxHashSet;

use super::{
    COL_NAME_TREE, EdgeWindow, ExpandMap, display_width, pad_str, print_data_cols,
    print_tree_header, truncate_str,
};
use crate::snapshot::HeapSnapshot;
use crate::types::NodeOrdinal;

pub fn print_containment(
    snap: &HeapSnapshot,
    node_ordinal: NodeOrdinal,
    max_depth: usize,
    expand: &ExpandMap,
) {
    let stats = snap.get_statistics();
    let total_shallow = stats.total;
    let total_retained = stats.total;

    let name = snap.node_display_name(node_ordinal);
    let id = snap.node_id(node_ordinal);
    println!("\nContainment for {name} @{id}:");
    print_tree_header(COL_NAME_TREE);

    fn edge_window(
        snap: &HeapSnapshot,
        node_ordinal: NodeOrdinal,
        expand: &ExpandMap,
    ) -> EdgeWindow {
        expand
            .get(&snap.node_id(node_ordinal))
            .copied()
            .unwrap_or_default()
    }

    fn walk(
        snap: &HeapSnapshot,
        node_ordinal: NodeOrdinal,
        depth: usize,
        max_depth: usize,
        visited: &mut FxHashSet<NodeOrdinal>,
        expand: &ExpandMap,
        total_shallow: u64,
        total_retained: u64,
    ) {
        if depth > max_depth && !expand.contains_key(&snap.node_id(node_ordinal)) {
            return;
        }
        let w = edge_window(snap, node_ordinal, expand);
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

            let indent = "  ".repeat(depth);
            let has_children = snap.node_edge_count(child_ordinal) > 0;
            let should_expand = !visited.contains(&child_ordinal)
                && has_children
                && (depth < max_depth || expand.contains_key(&child_id));
            let marker = if should_expand {
                "\u{25bc} " /* ▼ */
            } else {
                "\u{25b6} " /* ▶ */
            };

            let label = snap.format_edge_label(edge_idx, child_ordinal);
            let prefix = format!("{indent}{marker}");
            let max_name_len = COL_NAME_TREE.saturating_sub(display_width(&prefix));
            let label = truncate_str(&label, max_name_len);
            let name_col = pad_str(&format!("{prefix}{label}"), COL_NAME_TREE);

            print_data_cols(
                &name_col,
                snap.node_distance(child_ordinal),
                snap.node_self_size(child_ordinal) as u64,
                snap.node_retained_size(child_ordinal),
                total_shallow,
                total_retained,
            );

            if should_expand
                || (!visited.contains(&child_ordinal) && expand.contains_key(&child_id))
            {
                visited.insert(child_ordinal);
                walk(
                    snap,
                    child_ordinal,
                    depth + 1,
                    max_depth,
                    visited,
                    expand,
                    total_shallow,
                    total_retained,
                );
                visited.remove(&child_ordinal);
            }
        }
        if shown < total_edges {
            let indent = "  ".repeat(depth);
            // \u{2013} = –
            println!(
                "{indent}  {}\u{2013}{} of {total_edges} refs",
                start + 1,
                start + shown
            );
        }
    }

    {
        let label = snap.format_node_label(node_ordinal);
        let label = truncate_str(&label, COL_NAME_TREE);
        let name_col = pad_str(&label, COL_NAME_TREE);
        print_data_cols(
            &name_col,
            snap.node_distance(node_ordinal),
            snap.node_self_size(node_ordinal) as u64,
            snap.node_retained_size(node_ordinal),
            total_shallow,
            total_retained,
        );
    }
    let mut visited: FxHashSet<NodeOrdinal> = FxHashSet::default();
    visited.insert(node_ordinal);
    walk(
        snap,
        node_ordinal,
        0,
        max_depth,
        &mut visited,
        expand,
        total_shallow,
        total_retained,
    );
}
