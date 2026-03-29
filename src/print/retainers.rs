use rustc_hash::FxHashMap;

use super::{
    COL_NAME_TREE, ExpandMap, display_width, pad_str, print_data_cols, print_tree_header,
    truncate_str,
};
use crate::snapshot::HeapSnapshot;
use crate::types::{NodeId, NodeOrdinal};

// Re-export so existing `use crate::print::retainers::…` paths keep working.
pub use crate::retaining_path::{
    RetainerAutoExpandLimits, RetainerAutoExpandPlan, RetainerPathEdge, plan_gc_root_retainer_paths,
};

pub fn print_retainers(
    snap: &HeapSnapshot,
    node_id: NodeId,
    _max_depth: usize,
    expand: &ExpandMap,
    object_col_width: Option<usize>,
    auto_limits: RetainerAutoExpandLimits,
) {
    let node_ordinal = match snap.node_for_snapshot_object_id(node_id) {
        Some(o) => o,
        None => {
            println!("Error: no node found with id @{node_id}");
            std::process::exit(1);
        }
    };

    let auto = plan_gc_root_retainer_paths(snap, node_ordinal, auto_limits);

    let stats = snap.get_statistics();
    let total_shallow = stats.total;
    let total_retained = stats.total;

    let name = snap.node_display_name(node_ordinal);
    let id = snap.node_id(node_ordinal);
    let col_name_tree = object_col_width.unwrap_or(COL_NAME_TREE);
    println!("\nRetainers for {name} @{id}:");
    print_tree_header(col_name_tree);
    if auto.truncated {
        println!(
            "(auto-expansion hit limits; current --max-depth={} --max-nodes={}. Increase them to traverse more retainers)",
            auto_limits.max_depth, auto_limits.max_nodes
        );
    }
    if !auto.reached_gc_roots && !snap.is_root(node_ordinal) {
        println!("(no retainer path to (GC roots) found within current limits)");
    }

    fn print_row(
        snap: &HeapSnapshot,
        edge_idx: usize,
        ret_ordinal: NodeOrdinal,
        depth: usize,
        expanded: bool,
        total_shallow: f64,
        total_retained: f64,
        col_name_tree: usize,
    ) {
        let edge_name = snap.edge_name(edge_idx);
        let edge_type = snap.edge_type_name(edge_idx);
        let node_name = snap.node_display_name(ret_ordinal);
        let nid = snap.node_id(ret_ordinal);

        let indent = "  ".repeat(depth);
        let marker = if expanded {
            "\u{25bc} " /* ▼ */
        } else {
            "\u{25b6} " /* ▶ */
        };

        let label = if edge_type == "element" || edge_type == "hidden" {
            format!("[{edge_name}] in {node_name} @{nid}")
        } else {
            format!("{edge_name} in {node_name} @{nid}")
        };

        let max_name_len =
            col_name_tree.saturating_sub(display_width(&format!("{indent}{marker}")));
        let label = truncate_str(&label, max_name_len);
        let name_col = pad_str(&format!("{indent}{marker}{label}"), col_name_tree);

        print_data_cols(
            &name_col,
            snap.node_distance(ret_ordinal),
            snap.node_self_size(ret_ordinal) as f64,
            snap.node_retained_size(ret_ordinal),
            total_shallow,
            total_retained,
        );
    }

    /// Walk the already-pruned plan tree, printing each node.
    fn walk_plan(
        snap: &HeapSnapshot,
        plan_edges: &[RetainerPathEdge],
        depth: usize,
        total_shallow: f64,
        total_retained: f64,
        col_name_tree: usize,
    ) {
        for pe in plan_edges {
            let expanded = !pe.children.is_empty();
            print_row(
                snap,
                pe.edge_idx,
                pe.retainer,
                depth,
                expanded,
                total_shallow,
                total_retained,
                col_name_tree,
            );
            if expanded {
                walk_plan(
                    snap,
                    &pe.children,
                    depth + 1,
                    total_shallow,
                    total_retained,
                    col_name_tree,
                );
            }
        }
    }

    // Index plan tree root entries by edge_idx.
    let plan_map: FxHashMap<usize, &RetainerPathEdge> =
        auto.tree.iter().map(|pe| (pe.edge_idx, pe)).collect();

    // Collect all direct retainers, sorted: plan-tree entries first (by
    // distance), then non-plan entries (by distance).
    let mut all_retainers = snap.get_retainers(node_ordinal);
    all_retainers.sort_by_key(|&(edge_idx, ret_ord)| {
        let on_plan = plan_map.contains_key(&edge_idx);
        (!on_plan, snap.node_distance(ret_ord))
    });

    let w = expand
        .get(&snap.node_id(node_ordinal))
        .copied()
        .unwrap_or_default();
    let total = all_retainers.len();
    let start = w.start.min(total);
    let end = (start + w.count).min(total);
    let shown = end - start;

    for &(edge_idx, ret_ordinal) in &all_retainers[start..end] {
        if let Some(pe) = plan_map.get(&edge_idx) {
            let expanded = !pe.children.is_empty();
            print_row(
                snap,
                edge_idx,
                ret_ordinal,
                0,
                expanded,
                total_shallow,
                total_retained,
                col_name_tree,
            );
            if expanded {
                walk_plan(
                    snap,
                    &pe.children,
                    1,
                    total_shallow,
                    total_retained,
                    col_name_tree,
                );
            }
        } else {
            // Off-plan retainer: show collapsed.
            print_row(
                snap,
                edge_idx,
                ret_ordinal,
                0,
                false,
                total_shallow,
                total_retained,
                col_name_tree,
            );
        }
    }

    if shown < total {
        println!("  {}\u{2013}{} of {total} refs", start + 1, start + shown);
    }
}
