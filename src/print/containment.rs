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

    fn print_node_line(
        snap: &HeapSnapshot,
        node_ordinal: NodeOrdinal,
        prefix: &str,
        total_shallow: f64,
        total_retained: f64,
    ) {
        let label = format!(
            "{} @{}",
            snap.node_display_name(node_ordinal),
            snap.node_id(node_ordinal)
        );
        let max_name_len = COL_NAME_TREE.saturating_sub(display_width(prefix));
        let label = truncate_str(&label, max_name_len);
        let name_col = pad_str(&format!("{prefix}{label}"), COL_NAME_TREE);

        print_data_cols(
            &name_col,
            snap.node_distance(node_ordinal),
            snap.node_self_size(node_ordinal) as f64,
            snap.node_retained_size(node_ordinal),
            total_shallow,
            total_retained,
        );
    }

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
        total_shallow: f64,
        total_retained: f64,
    ) {
        if depth > max_depth && !expand.contains_key(&snap.node_id(node_ordinal)) {
            return;
        }
        let w = edge_window(snap, node_ordinal, expand);
        let edges: Vec<_> = snap
            .get_edges(node_ordinal)
            .into_iter()
            .filter(|&(edge_idx, _)| !snap.is_invisible_edge(edge_idx))
            .collect();
        let total_edges = edges.len();
        let start = w.start.min(total_edges);
        let end = (start + w.count).min(total_edges);
        let shown = end - start;

        for (edge_idx, child_ordinal) in edges.into_iter().skip(start).take(w.count) {
            let edge_name = snap.edge_name(edge_idx);
            let edge_type = snap.edge_type_name(edge_idx);
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

            let edge_label = if edge_type == "element" || edge_type == "hidden" {
                format!("[{edge_name}]")
            } else {
                edge_name
            };

            let prefix = format!("{indent}{marker}{edge_label} :: ");
            print_node_line(snap, child_ordinal, &prefix, total_shallow, total_retained);

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

    print_node_line(snap, node_ordinal, "", total_shallow, total_retained);
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
