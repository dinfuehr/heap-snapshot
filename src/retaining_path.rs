use rustc_hash::{FxHashMap, FxHashSet};

use crate::snapshot::HeapSnapshot;
use crate::types::NodeOrdinal;

#[derive(Clone, Copy, Debug)]
pub struct RetainerAutoExpandLimits {
    pub max_depth: usize,
    pub max_nodes: usize,
}

/// A single retainer edge on a GC-root path.
pub struct RetainerPathEdge {
    pub edge_idx: usize,
    pub retainer: NodeOrdinal,
    /// Children of this retainer that are also on a GC-root path.
    /// Empty for GC root nodes (leaf of the retainer tree).
    pub children: Vec<RetainerPathEdge>,
}

pub struct RetainerAutoExpandPlan {
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
pub fn plan_gc_root_retainer_paths(
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
