use std::cell::Cell;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::snapshot::HeapSnapshot;
use crate::types::{AggregateInfo, Distance, NodeOrdinal};

use super::SummaryFilterMode;
use super::contains_ignore_case;
use super::types::*;

/// Insert allocation stack frames as pseudo-children at the given position.
fn insert_allocation_stack(
    snap: &HeapSnapshot,
    ord: NodeOrdinal,
    children: &mut Vec<ChildNode>,
    pos: usize,
    next_id: &Cell<u64>,
) {
    if let Some(stack) = snap.get_allocation_stack(ord) {
        for (i, frame) in stack.iter().enumerate() {
            let label = HeapSnapshot::format_allocation_frame(frame);
            let prefix = if i == 0 { "alloc: " } else { "    <- " };
            children.insert(
                pos + i,
                ChildNode {
                    id: mint_id(next_id),
                    label: format!("{prefix}{label}").into(),
                    distance: None,
                    shallow_size: 0,
                    retained_size: 0,
                    node_ordinal: None,
                    has_children: false,
                    children_key: None,
                    is_weak: false,
                    is_root_holder: false,
                },
            );
        }
    }
}

/// Format an outgoing edge label: `edge :: @id name`
pub(super) fn format_edge_label(
    snap: &HeapSnapshot,
    edge_idx: usize,
    child_ord: NodeOrdinal,
) -> String {
    snap.format_edge_label(edge_idx, child_ord)
}

/// Format a retainer edge label: `edge in @id name`
pub(super) fn format_retainer_label(
    snap: &HeapSnapshot,
    edge_idx: usize,
    ret_ord: NodeOrdinal,
) -> String {
    snap.format_retainer_label(edge_idx, ret_ord)
}

pub(super) fn compute_children(
    key: &ChildrenKey,
    row_id: NodeId,
    snap: &HeapSnapshot,
    sorted_aggregates: &[AggregateInfo],
    edge_windows: &FxHashMap<NodeId, EdgeWindow>,
    class_member_windows: &FxHashMap<usize, EdgeWindow>,
    edge_filters: &FxHashMap<NodeOrdinal, String>,
    summary_filter: &str,
    retainer_path_edges: Option<&FxHashSet<usize>>,
    unreachable_filter: SummaryFilterMode,
    next_id: &Cell<u64>,
) -> Vec<ChildNode> {
    match key {
        ChildrenKey::ClassMembers(i) => {
            let w = class_member_windows.get(i).copied().unwrap_or_default();
            compute_class_members(
                snap,
                &sorted_aggregates[*i],
                w,
                summary_filter,
                unreachable_filter,
                next_id,
            )
        }
        ChildrenKey::Edges(_, ord) => {
            let w = edge_windows.get(&row_id).copied().unwrap_or_default();
            let filter = edge_filters.get(ord).map(|s| s.as_str()).unwrap_or("");
            compute_edges(snap, *ord, w, filter, next_id)
        }
        ChildrenKey::Retainers(id, ord) => {
            let w = edge_windows.get(id).copied().unwrap_or_default();
            compute_retainers(snap, *ord, w, retainer_path_edges, next_id)
        }
        ChildrenKey::DominatedChildren(ord) => compute_dominated_children(snap, *ord, next_id),
        ChildrenKey::CompareEdges(..) | ChildrenKey::DiffMembers(_) => {
            // CompareEdges/DiffMembers handled specially in expand().
            Vec::new()
        }
    }
}

/// Check whether a node passes the unreachable filter.
fn passes_unreachable_filter(
    snap: &HeapSnapshot,
    ord: NodeOrdinal,
    filter: SummaryFilterMode,
) -> bool {
    // Aggregates are pre-filtered by set_summary_filter, so all members pass.
    let _ = (snap, ord, filter);
    true
}

pub(super) fn compute_class_members(
    snap: &HeapSnapshot,
    agg: &AggregateInfo,
    w: EdgeWindow,
    filter: &str,
    unreachable_filter: SummaryFilterMode,
    next_id: &Cell<u64>,
) -> Vec<ChildNode> {
    let _ = unreachable_filter; // Aggregates are pre-filtered by set_summary_filter
    let unreachable_active = false;
    let is_filtered = !filter.is_empty() || unreachable_active;
    if is_filtered {
        // Filter first, then page the matching subset.
        let matching: Vec<&NodeOrdinal> = agg
            .node_ordinals
            .iter()
            .filter(|ord| {
                if !passes_unreachable_filter(snap, **ord, unreachable_filter) {
                    return false;
                }
                filter.is_empty() || contains_ignore_case(snap.node_raw_name(**ord), filter)
            })
            .collect();
        let total = matching.len();
        let start = w.start.min(total);
        let end = (start + w.count).min(total);
        let shown = end - start;

        let mut children: Vec<ChildNode> = matching[start..end]
            .iter()
            .map(|&&ordinal| {
                let name = snap.node_display_name(ordinal);
                let node_id = snap.node_id(ordinal);
                let has_children = snap.node_edge_count(ordinal) > 0;
                let id = mint_id(next_id);
                ChildNode {
                    id,
                    label: format!("{name} @{node_id}").into(),
                    distance: Some(snap.node_distance(ordinal)),
                    shallow_size: snap.node_self_size(ordinal) as u64,
                    retained_size: snap.node_retained_size(ordinal),
                    node_ordinal: Some(ordinal),
                    has_children,
                    children_key: if has_children {
                        Some(ChildrenKey::Edges(id, ordinal))
                    } else {
                        None
                    },
                    is_weak: false,
                    is_root_holder: false,
                }
            })
            .collect();

        if shown < total {
            children.push(ChildNode {
                id: mint_id(next_id),
                label: format!(
                    "{}\u{2013}{} of {total} matching \"{filter}\"  (n/p: page, a: all)",
                    start + 1,
                    start + shown,
                )
                .into(),
                distance: None,
                shallow_size: 0,
                retained_size: 0,
                node_ordinal: None,
                has_children: false,
                children_key: None,
                is_weak: false,
                is_root_holder: false,
            });
        }
        return children;
    }

    let total = agg.node_ordinals.len();
    let start = w.start.min(total);
    let end = (start + w.count).min(total);

    let mut children: Vec<ChildNode> = agg.node_ordinals[start..end]
        .iter()
        .map(|&ordinal| {
            let name = snap.node_display_name(ordinal);
            let node_id = snap.node_id(ordinal);
            let has_children = snap.node_edge_count(ordinal) > 0;
            let id = mint_id(next_id);
            ChildNode {
                id,
                label: format!("{name} @{node_id}").into(),
                distance: Some(snap.node_distance(ordinal)),
                shallow_size: snap.node_self_size(ordinal) as u64,
                retained_size: snap.node_retained_size(ordinal),
                node_ordinal: Some(ordinal),
                has_children,
                children_key: if has_children {
                    Some(ChildrenKey::Edges(id, ordinal))
                } else {
                    None
                },
                is_weak: false,
                is_root_holder: false,
            }
        })
        .collect();

    let shown = end - start;
    if shown < total {
        children.push(ChildNode {
            id: mint_id(next_id),
            label: format!(
                "{}\u{2013}{} of {total} objects  (n/p: page, a: all)",
                start + 1,
                start + shown,
            )
            .into(),
            distance: None,
            shallow_size: 0,
            retained_size: 0,
            node_ordinal: None,
            has_children: false,
            children_key: None,
            is_weak: false,
            is_root_holder: false,
        });
    }

    children
}

/// Build a `ChildNode` from an edge, constructing the display label.
fn edge_to_child_node(
    snap: &HeapSnapshot,
    edge_idx: usize,
    child_ord: NodeOrdinal,
    next_id: &Cell<u64>,
) -> ChildNode {
    let edge_type = snap.edge_type_name(edge_idx);
    let label = format_edge_label(snap, edge_idx, child_ord);
    let has_children = snap.node_edge_count(child_ord) > 0;
    let id = mint_id(next_id);
    ChildNode {
        id,
        label: label.into(),
        distance: Some(snap.node_distance(child_ord)),
        shallow_size: snap.node_self_size(child_ord) as u64,
        retained_size: snap.node_retained_size(child_ord),
        node_ordinal: Some(child_ord),
        has_children,
        children_key: if has_children {
            Some(ChildrenKey::Edges(id, child_ord))
        } else {
            None
        },
        is_weak: edge_type == "weak",
        is_root_holder: false,
    }
}

pub(super) fn compute_edges(
    snap: &HeapSnapshot,
    ord: NodeOrdinal,
    w: EdgeWindow,
    filter: &str,
    next_id: &Cell<u64>,
) -> Vec<ChildNode> {
    let is_native_ctx = snap.is_native_context(ord);
    let needs_sort = is_native_ctx || snap.is_js_global_object(ord) || snap.is_js_global_proxy(ord);

    // Fast path: no filter and no custom sort — page raw edges directly,
    // only build labels for the visible slice.  Two passes over the edge
    // array (count + skip/take) avoids allocating a Vec for all edges.
    if filter.is_empty() && !needs_sort {
        let total = snap
            .iter_edges(ord)
            .filter(|&(edge_idx, _)| !snap.is_invisible_edge(edge_idx))
            .count();
        let start = w.start.min(total);
        let end = (start + w.count).min(total);
        let visible = end - start;

        let mut children: Vec<ChildNode> = snap
            .iter_edges(ord)
            .filter(|&(edge_idx, _)| !snap.is_invisible_edge(edge_idx))
            .skip(start)
            .take(w.count)
            .map(|(edge_idx, child_ord)| edge_to_child_node(snap, edge_idx, child_ord, next_id))
            .collect();

        if visible < total {
            children.push(ChildNode {
                id: mint_id(next_id),
                label: format!(
                    "{}\u{2013}{} of {total} refs  (n/p: page, a: all)",
                    start + 1,
                    start + visible,
                )
                .into(),
                distance: None,
                shallow_size: 0,
                retained_size: 0,
                node_ordinal: None,
                has_children: false,
                children_key: None,
                is_weak: false,
                is_root_holder: false,
            });
        }
        // For JSFunction / SharedFunctionInfo nodes, prepend a location info row.
        if snap.is_js_function(ord) || snap.is_shared_function_info(ord) {
            if let Some(loc) = snap.node_location(ord) {
                let label = snap.format_location(&loc);
                children.insert(
                    0,
                    ChildNode {
                        id: mint_id(next_id),
                        label: label.into(),
                        distance: None,
                        shallow_size: 0,
                        retained_size: 0,
                        node_ordinal: None,
                        has_children: false,
                        children_key: None,
                        is_weak: false,
                        is_root_holder: false,
                    },
                );
            }
        }

        // Insert allocation stack after location (or at position 0)
        let alloc_pos = if children.first().map_or(false, |c| c.node_ordinal.is_none()) {
            1
        } else {
            0
        };
        insert_allocation_stack(snap, ord, &mut children, alloc_pos, next_id);

        return children;
    }

    // Slow path: filter or custom sort requires building all labels first.
    let mut all: Vec<(String, ChildNode)> = snap
        .iter_edges(ord)
        .filter(|&(edge_idx, _)| !snap.is_invisible_edge(edge_idx))
        .map(|(edge_idx, child_ord)| {
            let edge_name = snap.edge_name(edge_idx);
            let child = edge_to_child_node(snap, edge_idx, child_ord, next_id);
            (edge_name, child)
        })
        .filter(|(_name, c)| filter.is_empty() || contains_ignore_case(&c.label, filter))
        .collect();

    // For NativeContext nodes, show pinned fields first, rest after.
    if is_native_ctx {
        const PINNED: &[&str] = &[
            "scope_info",
            "global_object",
            "global_proxy_object",
            "script_context_table",
        ];
        all.sort_by_key(|(name, _)| {
            if let Some(pos) = PINNED.iter().position(|&p| p == name) {
                (0, pos)
            } else {
                (1, 0)
            }
        });
    } else if snap.is_js_global_object(ord) || snap.is_js_global_proxy(ord) {
        all.sort_by_key(|(name, _)| snap.is_common_js_global_field(ord, name));
    }

    let total = all.len();
    let start = w.start.min(total);
    let end = (start + w.count).min(total);
    let visible = end - start;

    let mut children: Vec<ChildNode> = all
        .into_iter()
        .skip(start)
        .take(w.count)
        .map(|(_, c)| c)
        .collect();

    // For NativeContext nodes, prepend a Vars info row.
    if is_native_ctx {
        let vars = snap.native_context_vars(ord);
        if !vars.is_empty() {
            let label = format!("Vars: {vars}");
            children.insert(
                0,
                ChildNode {
                    id: mint_id(next_id),
                    label: label.into(),
                    distance: None,
                    shallow_size: 0,
                    retained_size: 0,
                    node_ordinal: None,
                    has_children: false,
                    children_key: None,
                    is_weak: false,
                    is_root_holder: false,
                },
            );
        }
    }

    // For JSFunction / SharedFunctionInfo nodes, prepend a location info row.
    if snap.is_js_function(ord) || snap.is_shared_function_info(ord) {
        if let Some(loc) = snap.node_location(ord) {
            let label = snap.format_location(&loc);
            children.insert(
                0,
                ChildNode {
                    id: mint_id(next_id),
                    label: label.into(),
                    distance: None,
                    shallow_size: 0,
                    retained_size: 0,
                    node_ordinal: None,
                    has_children: false,
                    children_key: None,
                    is_weak: false,
                    is_root_holder: false,
                },
            );
        }
    }

    // Insert allocation stack after location (or at position 0)
    let alloc_pos = if children.first().map_or(false, |c| c.node_ordinal.is_none()) {
        1
    } else {
        0
    };
    insert_allocation_stack(snap, ord, &mut children, alloc_pos, next_id);

    // Status line: show range and filter info
    if total > 0 && (visible < total || !filter.is_empty()) {
        let range = format!("{}\u{2013}{}", start + 1, start + visible);
        let mut status = format!("{range} of {total} refs");
        if !filter.is_empty() {
            status.push_str(&format!(" matching \"{filter}\""));
        }
        if visible < total {
            status.push_str("  (n/p: page, a: all)");
        }
        children.push(ChildNode {
            id: mint_id(next_id),
            label: status.into(),
            distance: None,
            shallow_size: 0,
            retained_size: 0,
            node_ordinal: None,
            has_children: false,
            children_key: None,
            is_weak: false,
            is_root_holder: false,
        });
    }

    children
}

/// Like `compute_edges` but tags child nodes with `CompareEdges` instead of `Edges`,
/// so edge expansion continues to resolve against the compare snapshot.
pub(super) fn compute_compare_edges(
    snap: &HeapSnapshot,
    ord: NodeOrdinal,
    w: EdgeWindow,
    filter: &str,
    next_id: &Cell<u64>,
) -> Vec<ChildNode> {
    let mut children = compute_edges(snap, ord, w, filter, next_id);
    for child in &mut children {
        if let Some(ChildrenKey::Edges(id, o)) = &child.children_key {
            child.children_key = Some(ChildrenKey::CompareEdges(*id, *o));
        }
    }
    children
}

/// Build a `ChildNode` for a single retainer edge.
pub(super) fn make_retainer_child(
    snap: &HeapSnapshot,
    edge_idx: usize,
    ret_ord: NodeOrdinal,
    next_id: &Cell<u64>,
) -> ChildNode {
    let edge_type = snap.edge_type_name(edge_idx);
    let is_weak = edge_type == "weak";
    let label = format_retainer_label(snap, edge_idx, ret_ord);
    let dist = snap.node_distance(ret_ord);
    let expandable = dist > Distance(0) && !dist.is_unreachable();
    ChildNode {
        id: mint_id(next_id),
        label: label.into(),
        distance: Some(dist),
        shallow_size: snap.node_self_size(ret_ord) as u64,
        retained_size: snap.node_retained_size(ret_ord),
        node_ordinal: Some(ret_ord),
        has_children: expandable,
        children_key: if expandable {
            Some(ChildrenKey::Retainers(mint_id(next_id), ret_ord))
        } else {
            None
        },
        is_weak,
        is_root_holder: snap.is_root_holder(ret_ord),
    }
}

pub(super) fn compute_retainers(
    snap: &HeapSnapshot,
    ord: NodeOrdinal,
    w: EdgeWindow,
    path_edges: Option<&FxHashSet<usize>>,
    next_id: &Cell<u64>,
) -> Vec<ChildNode> {
    let make_child = |edge_idx: usize, ret_ord: NodeOrdinal, next_id: &Cell<u64>| {
        make_retainer_child(snap, edge_idx, ret_ord, next_id)
    };

    let start = w.start;

    // When a path_edges filter is active we need to sort all matching
    // retainers by distance before paging, so closer-to-root retainers
    // always appear on earlier pages.
    if path_edges.is_some() {
        let mut all: Vec<(usize, NodeOrdinal)> = Vec::new();
        snap.for_each_retainer(ord, |edge_idx, ret_ord| {
            let pe = path_edges.unwrap();
            if !pe.contains(&edge_idx) {
                return;
            }
            all.push((edge_idx, ret_ord));
        });
        all.sort_by_key(|&(_, ret_ord)| snap.node_distance(ret_ord));
        let total = all.len();
        let page_start = start.min(total);
        let page_end = (page_start + w.count).min(total);
        let mut children: Vec<ChildNode> = all[page_start..page_end]
            .iter()
            .map(|&(edge_idx, ret_ord)| make_child(edge_idx, ret_ord, next_id))
            .collect();
        let visible = children.len();
        if total > 0 && (visible < total || start > 0) {
            let shown_start = page_start + 1;
            let shown_end = page_start + visible;
            children.push(ChildNode {
                id: mint_id(next_id),
                label: if visible < total {
                    format!(
                        "{shown_start}\u{2013}{shown_end} of {total} retainers  (n/p: page, a: all)"
                    )
                } else {
                    format!("{shown_start}\u{2013}{shown_end} of {total} retainers")
                }
                .into(),
                distance: None,
                shallow_size: 0,
                retained_size: 0,
                node_ordinal: None,
                has_children: false,
                children_key: None,
                is_weak: false,
                is_root_holder: false,
            });
        }
        return children;
    }

    // Unfiltered path: page directly from the raw iteration order.
    let mut total = 0usize;
    let mut children = Vec::new();
    snap.for_each_retainer(ord, |edge_idx, ret_ord| {
        let idx = total;
        total += 1;
        if idx < start || children.len() >= w.count {
            return;
        }
        children.push(make_child(edge_idx, ret_ord, next_id));
    });

    let visible = children.len();
    if total > 0 && (visible < total || start > 0) {
        let shown_start = start.min(total) + 1;
        let shown_end = start + visible;
        children.push(ChildNode {
            id: mint_id(next_id),
            label: if visible < total {
                format!(
                    "{shown_start}\u{2013}{shown_end} of {total} retainers  (n/p: page, a: all)"
                )
            } else {
                format!("{shown_start}\u{2013}{shown_end} of {total} retainers")
            }
            .into(),
            distance: None,
            shallow_size: 0,
            retained_size: 0,
            node_ordinal: None,
            has_children: false,
            children_key: None,
            is_weak: false,
            is_root_holder: false,
        });
    }
    children
}

pub(super) fn compute_dominated_children(
    snap: &HeapSnapshot,
    ord: NodeOrdinal,
    next_id: &Cell<u64>,
) -> Vec<ChildNode> {
    let mut children: Vec<ChildNode> = snap
        .get_dominated_children(ord)
        .into_iter()
        .map(|child_ord| {
            let display_name = snap.node_display_name(child_ord);
            let node_id = snap.node_id(child_ord);
            let has_children = !snap.get_dominated_children(child_ord).is_empty();
            ChildNode {
                id: mint_id(next_id),
                label: format!("{display_name} @{node_id}").into(),
                distance: Some(snap.node_distance(child_ord)),
                shallow_size: snap.node_self_size(child_ord) as u64,
                retained_size: snap.node_retained_size(child_ord),
                node_ordinal: Some(child_ord),
                has_children,
                children_key: if has_children {
                    Some(ChildrenKey::DominatedChildren(child_ord))
                } else {
                    None
                },
                is_weak: false,
                is_root_holder: false,
            }
        })
        .collect();
    // Sort by retained size descending
    children.sort_by(|a, b| b.retained_size.cmp(&a.retained_size));
    children
}

// Compute the new window start after shifting by `delta`.
// Allows partial last pages but won't advance past them.
pub(super) fn shifted_window_start(
    start: usize,
    count: usize,
    total: usize,
    delta: isize,
) -> usize {
    let max_start = if start + count >= total {
        start
    } else {
        total.saturating_sub(1)
    };
    (start as isize + delta).max(0).min(max_start as isize) as usize
}
