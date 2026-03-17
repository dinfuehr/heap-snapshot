use rustc_hash::{FxHashMap, FxHashSet};

use super::{
    COL_NAME_TREE, ExpandMap, display_width, pad_str, print_data_cols, print_tree_header,
    truncate_str,
};
use crate::snapshot::HeapSnapshot;
use crate::types::{NodeId, NodeOrdinal};

#[derive(Clone, Copy, Debug)]
pub struct RetainerAutoExpandLimits {
    pub max_depth: usize,
    pub max_nodes: usize,
}

/// A single retainer edge on a GC-root path.
pub(crate) struct RetainerPathEdge {
    pub edge_idx: usize,
    pub retainer: NodeOrdinal,
    /// Children of this retainer that are also on a GC-root path.
    /// Empty for GC root nodes (leaf of the retainer tree).
    pub children: Vec<RetainerPathEdge>,
}

pub(crate) struct RetainerAutoExpandPlan {
    /// Tree of retainer edges from target to GC roots.
    /// Each entry is a direct retainer of the target node.
    /// Note: shared subgraphs are pruned — a node that appears on multiple
    /// paths has its subtree built only once; subsequent occurrences have
    /// empty `children` but are still marked as reaching GC roots.
    pub tree: Vec<RetainerPathEdge>,
    pub gc_root_path_edges: FxHashSet<usize>,
    pub reached_gc_roots: bool,
    pub truncated: bool,
}

/// Collect all retainer edges that lie on at least one path from `start` to
/// `(GC roots)`, bounded by `limits`.  Returns a tree of
/// [`RetainerPathEdge`]s ready for the TUI to convert to `ChildNode`s.
pub(crate) fn plan_gc_root_retainer_paths(
    snap: &HeapSnapshot,
    start: NodeOrdinal,
    limits: RetainerAutoExpandLimits,
) -> RetainerAutoExpandPlan {
    let mut gc_root_path_edges = FxHashSet::default();
    let mut explored_nodes = FxHashSet::default();
    let mut dfs_stack = FxHashSet::default();
    // Memo stores (reaches_gc_roots, depth_explored).  `true` is always
    // reusable.  `false` is only reusable when the current depth >= the
    // memoized depth (i.e. we have equal or less remaining budget), because
    // a shallower encounter has more remaining depth budget and might
    // succeed where the deeper one failed.
    let mut memo: FxHashMap<NodeOrdinal, (bool, usize)> = FxHashMap::default();
    let mut truncated = false;

    /// Returns the subtree of retainer path edges for `node`, or empty if
    /// no path to GC roots was found within budget.
    fn dfs(
        snap: &HeapSnapshot,
        node: NodeOrdinal,
        depth: usize,
        limits: RetainerAutoExpandLimits,
        explored_nodes: &mut FxHashSet<NodeOrdinal>,
        dfs_stack: &mut FxHashSet<NodeOrdinal>,
        memo: &mut FxHashMap<NodeOrdinal, (bool, usize)>,
        gc_root_path_edges: &mut FxHashSet<usize>,
        truncated: &mut bool,
    ) -> Vec<RetainerPathEdge> {
        if snap.is_root(node) {
            return Vec::new(); // leaf — GC root reached
        }
        if depth >= limits.max_depth {
            return Vec::new();
        }
        if let Some(&(_result, explored_depth)) = memo.get(&node) {
            if depth >= explored_depth {
                // Same or deeper visit — the previous exploration had equal
                // or more remaining budget, so it already found everything
                // reachable within limits.  Reuse the memo.
                return Vec::new();
            }
            // Shallower visit with more remaining budget — re-explore to
            // collect any gc_root_path_edges the deeper visit couldn't reach.
        }
        if !explored_nodes.contains(&node) {
            if explored_nodes.len() >= limits.max_nodes {
                *truncated = true;
                return Vec::new();
            }
            explored_nodes.insert(node);
        }
        if !dfs_stack.insert(node) {
            return Vec::new();
        }

        let mut retainers: Vec<(usize, NodeOrdinal)> = Vec::new();
        let mut directly_retained_by_gc_roots = false;
        snap.for_each_retainer(node, |edge_idx, ret_ordinal| {
            if dfs_stack.contains(&ret_ordinal) {
                return;
            }
            if snap.is_root(ret_ordinal) {
                // This node is directly retained by (GC roots) — it is a
                // root category like (Strong roots) or (Global handles).
                // Don't recurse into (GC roots); just mark this node as
                // reaching GC roots so it becomes the leaf of the path.
                directly_retained_by_gc_roots = true;
                gc_root_path_edges.insert(edge_idx);
                return;
            }
            retainers.push((edge_idx, ret_ordinal));
        });
        retainers.sort_by_key(|(_, ord)| snap.node_distance(*ord));

        let mut result = Vec::new();
        let mut reaches_gc_roots = directly_retained_by_gc_roots;
        for (edge_idx, ret_ordinal) in retainers {
            let children = dfs(
                snap,
                ret_ordinal,
                depth + 1,
                limits,
                explored_nodes,
                dfs_stack,
                memo,
                gc_root_path_edges,
                truncated,
            );
            let child_reaches =
                !children.is_empty() || memo.get(&ret_ordinal).is_some_and(|&(r, _)| r);
            if child_reaches {
                reaches_gc_roots = true;
                gc_root_path_edges.insert(edge_idx);
                // Only add to the visible plan tree when there is actual
                // subtree content or the node is a root holder (leaf of
                // the path).  Shared-subgraph-pruned nodes (memo says
                // reachable but children is empty and not a root holder)
                // would appear as dead-end branches in auto-expand — skip
                // them.  Their edges are still in gc_root_path_edges so
                // manual expand / filtering works correctly.
                let is_root_holder = snap.is_root_holder(ret_ordinal);
                if !children.is_empty() || is_root_holder {
                    result.push(RetainerPathEdge {
                        edge_idx,
                        retainer: ret_ordinal,
                        children,
                    });
                }
            }
        }

        dfs_stack.remove(&node);
        memo.insert(node, (reaches_gc_roots, depth));
        result
    }

    let tree = dfs(
        snap,
        start,
        0,
        limits,
        &mut explored_nodes,
        &mut dfs_stack,
        &mut memo,
        &mut gc_root_path_edges,
        &mut truncated,
    );
    let reached_gc_roots =
        !tree.is_empty() || snap.is_root(start) || memo.get(&start).is_some_and(|&(r, _)| r);

    // Prune the plan tree so that every path leads to at least one
    // not-yet-seen root-holder leaf.  Intermediate nodes are deduplicated
    // (expanded only on the first path), root-holder leaves are always
    // kept (different edges to the same root holder are all shown), and
    // each edge_idx appears at most once.  This makes the tree ready for
    // direct iteration by both the CLI and TUI without further dedup.
    let mut expanded_once: FxHashSet<NodeOrdinal> = FxHashSet::default();
    let mut seen_edges: FxHashSet<usize> = FxHashSet::default();
    let tree = prune_plan_tree(snap, tree, &mut expanded_once, &mut seen_edges);

    RetainerAutoExpandPlan {
        tree,
        gc_root_path_edges,
        reached_gc_roots,
        truncated,
    }
}

/// Returns true if the plan subtree rooted at `pe` contains at least one
/// root-holder leaf whose ordinal hasn't been expanded yet.
fn has_new_leaf(
    snap: &HeapSnapshot,
    pe: &RetainerPathEdge,
    expanded_once: &FxHashSet<NodeOrdinal>,
) -> bool {
    if expanded_once.contains(&pe.retainer) {
        return false;
    }
    if snap.is_root_holder(pe.retainer) {
        return true;
    }
    pe.children
        .iter()
        .any(|child| has_new_leaf(snap, child, expanded_once))
}

/// Prune a plan tree: keep only subtrees that reach a not-yet-seen
/// root-holder leaf.  Intermediate nodes go into `expanded_once` so
/// subsequent duplicates are pruned.  Each edge_idx appears at most once.
fn prune_plan_tree(
    snap: &HeapSnapshot,
    edges: Vec<RetainerPathEdge>,
    expanded_once: &mut FxHashSet<NodeOrdinal>,
    seen_edges: &mut FxHashSet<usize>,
) -> Vec<RetainerPathEdge> {
    let mut result = Vec::new();
    for mut pe in edges {
        if !has_new_leaf(snap, &pe, expanded_once) {
            continue;
        }
        if !seen_edges.insert(pe.edge_idx) {
            continue;
        }
        if !pe.children.is_empty() {
            expanded_once.insert(pe.retainer);
            pe.children = prune_plan_tree(snap, pe.children, expanded_once, seen_edges);
        }
        result.push(pe);
    }
    result
}

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
