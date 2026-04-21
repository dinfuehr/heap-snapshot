use crate::print::retainers::RetainerAutoExpandPlan;
use crate::snapshot::HeapSnapshot;
use crate::types::{Distance, NodeOrdinal};

use super::children;
use super::children::{
    compute_children, compute_class_members, compute_compare_edges, compute_edges,
    compute_retainers, make_retainer_child, shifted_window_start,
};
use super::types::*;
use super::{
    App, EDGE_PAGE_SIZE, RETAINER_AUTO_EXPAND_DEPTH, RETAINER_AUTO_EXPAND_NODES, SummaryFilterMode,
    contains_ignore_case,
};

impl App {
    pub(super) fn apply_retainers_plan(
        &mut self,
        ordinal: NodeOrdinal,
        plan: RetainerAutoExpandPlan,
        snap: &HeapSnapshot,
    ) {
        use crate::print::retainers::RetainerPathEdge;
        use rustc_hash::FxHashMap;

        self.retainers.gc_root_path_edges = plan.gc_root_path_edges;

        // Build deeper levels of the already-pruned plan tree.  Each node
        // gets a unique NodeId so the TUI tree has no shared subtrees.
        fn build_subtree(
            snap: &HeapSnapshot,
            retainer_ord: NodeOrdinal,
            plan_edges: &[RetainerPathEdge],
            state: &mut TreeState,
            next_id: &std::cell::Cell<u64>,
        ) -> Vec<ChildNode> {
            let mut children = Vec::new();
            for pe in plan_edges {
                let edge_type = snap.edge_type_name(pe.edge_idx);
                let is_weak = edge_type == "weak";
                let label = children::format_retainer_label(snap, pe.edge_idx, pe.retainer);
                let dist = snap.node_distance(pe.retainer);
                let id = mint_id(next_id);
                let has_children = dist > Distance(0);

                let children_key = if !pe.children.is_empty() && has_children {
                    let ck = ChildrenKey::Retainers(id, pe.retainer);
                    let sub = build_subtree(snap, pe.retainer, &pe.children, state, next_id);
                    state.expanded.insert(id);
                    state.children_map.insert(ck.clone(), sub);
                    Some(ck)
                } else if has_children {
                    Some(ChildrenKey::Retainers(id, pe.retainer))
                } else {
                    None
                };

                children.push(ChildNode {
                    id,
                    label: label.into(),
                    distance: Some(dist),
                    shallow_size: snap.node_self_size(pe.retainer) as u64,
                    retained_size: snap.node_retained_size(pe.retainer),
                    node_ordinal: Some(pe.retainer),
                    has_children,
                    children_key,
                    is_weak,
                    is_root_holder: snap.is_root_holder(pe.retainer),
                    inspect_source: None,
                });
            }
            let selected = children.len();
            let total = snap.retainer_count(retainer_ord);
            if selected > 0 && selected < total {
                children.push(ChildNode {
                    id: mint_id(next_id),
                    label: format!("{selected} selected of {total} retainers  (v: view all)")
                        .into(),
                    distance: None,
                    shallow_size: 0,
                    retained_size: 0,
                    node_ordinal: None,
                    has_children: false,
                    children_key: None,
                    is_weak: false,
                    is_root_holder: false,
                    inspect_source: None,
                });
            }
            children
        }

        // Index plan tree root entries by edge_idx for fast lookup.
        let plan_map: FxHashMap<usize, &RetainerPathEdge> =
            plan.tree.iter().map(|pe| (pe.edge_idx, pe)).collect();

        // Collect ALL retainers, sorted: plan-tree entries first (by distance),
        // then non-plan entries (by distance).
        let mut all: Vec<(usize, NodeOrdinal)> = Vec::new();
        snap.for_each_retainer(ordinal, |edge_idx, ret_ord| {
            all.push((edge_idx, ret_ord));
        });
        all.sort_by_key(|&(edge_idx, ret_ord)| {
            let on_plan = plan_map.contains_key(&edge_idx);
            (!on_plan, snap.node_distance(ret_ord))
        });

        // Apply paging.
        let root_id = self
            .retainers
            .root_id
            .unwrap_or_else(|| mint_id(&self.next_id));
        let w = self
            .retainers
            .tree_state
            .edge_windows
            .get(&root_id)
            .copied()
            .unwrap_or_default();
        let total = all.len();
        let start = w.start.min(total);
        let end = (start + w.count).min(total);

        // Build root children from the already-pruned plan tree.
        let mut root_children = Vec::new();
        for &(edge_idx, ret_ord) in &all[start..end] {
            if let Some(pe) = plan_map.get(&edge_idx) {
                let edge_type = snap.edge_type_name(edge_idx);
                let is_weak = edge_type == "weak";
                let label = children::format_retainer_label(snap, edge_idx, ret_ord);
                let dist = snap.node_distance(ret_ord);
                let id = mint_id(&self.next_id);
                let has_children = dist > Distance(0);

                let children_key = if !pe.children.is_empty() && has_children {
                    let ck = ChildrenKey::Retainers(id, ret_ord);
                    let sub = build_subtree(
                        snap,
                        ret_ord,
                        &pe.children,
                        &mut self.retainers.tree_state,
                        &self.next_id,
                    );
                    self.retainers.tree_state.expanded.insert(id);
                    self.retainers
                        .tree_state
                        .children_map
                        .insert(ck.clone(), sub);
                    Some(ck)
                } else if has_children {
                    Some(ChildrenKey::Retainers(id, ret_ord))
                } else {
                    None
                };

                root_children.push(ChildNode {
                    id,
                    label: label.into(),
                    distance: Some(dist),
                    shallow_size: snap.node_self_size(ret_ord) as u64,
                    retained_size: snap.node_retained_size(ret_ord),
                    node_ordinal: Some(ret_ord),
                    has_children,
                    children_key,
                    is_weak,
                    is_root_holder: snap.is_root_holder(ret_ord),
                    inspect_source: None,
                });
            } else {
                root_children.push(make_retainer_child(snap, edge_idx, ret_ord, &self.next_id));
            }
        }

        // Paging status line.
        let visible = root_children.len();
        if total > 0 && (visible < total || start > 0) {
            let shown_start = start + 1;
            let shown_end = start + visible;
            root_children.push(ChildNode {
                id: mint_id(&self.next_id),
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
                inspect_source: None,
            });
        }

        let key = ChildrenKey::Retainers(root_id, ordinal);
        self.retainers
            .tree_state
            .children_map
            .insert(key, root_children);
        self.mark_rows_dirty();
    }

    fn apply_retainers_subtree_plan(
        &mut self,
        ordinal: NodeOrdinal,
        children_key: ChildrenKey,
        plan: RetainerAutoExpandPlan,
        snap: &HeapSnapshot,
    ) {
        self.retainers
            .gc_root_path_edges
            .extend(plan.gc_root_path_edges);

        // Rebuild the subtree from the plan tree — same logic as
        // apply_retainers_plan but for a subtree expansion.
        use crate::print::retainers::RetainerPathEdge;

        fn build_children(
            snap: &HeapSnapshot,
            retainer_ord: NodeOrdinal,
            plan_edges: &[RetainerPathEdge],
            state: &mut TreeState,
            next_id: &std::cell::Cell<u64>,
        ) -> Vec<ChildNode> {
            let mut children = Vec::new();
            for pe in plan_edges {
                let edge_type = snap.edge_type_name(pe.edge_idx);
                let is_weak = edge_type == "weak";
                let label = children::format_retainer_label(snap, pe.edge_idx, pe.retainer);
                let dist = snap.node_distance(pe.retainer);
                let id = mint_id(next_id);
                let has_children = dist > Distance(0);

                let children_key = if !pe.children.is_empty() && has_children {
                    let ck = ChildrenKey::Retainers(id, pe.retainer);
                    let sub = build_children(snap, pe.retainer, &pe.children, state, next_id);
                    state.expanded.insert(id);
                    state.children_map.insert(ck.clone(), sub);
                    Some(ck)
                } else if has_children {
                    Some(ChildrenKey::Retainers(id, pe.retainer))
                } else {
                    None
                };

                children.push(ChildNode {
                    id,
                    label: label.into(),
                    distance: Some(dist),
                    shallow_size: snap.node_self_size(pe.retainer) as u64,
                    retained_size: snap.node_retained_size(pe.retainer),
                    node_ordinal: Some(pe.retainer),
                    has_children,
                    children_key,
                    is_weak,
                    is_root_holder: snap.is_root_holder(pe.retainer),
                    inspect_source: None,
                });
            }
            let selected = children.len();
            let total = snap.retainer_count(retainer_ord);
            if selected > 0 && selected < total {
                children.push(ChildNode {
                    id: mint_id(next_id),
                    label: format!("{selected} selected of {total} retainers  (v: view all)")
                        .into(),
                    distance: None,
                    shallow_size: 0,
                    retained_size: 0,
                    node_ordinal: None,
                    has_children: false,
                    children_key: None,
                    is_weak: false,
                    is_root_holder: false,
                    inspect_source: None,
                });
            }
            children
        }

        // Drop old cached children for this subtree.
        self.retainers.tree_state.children_map.remove(&children_key);

        let root_children = build_children(
            snap,
            ordinal,
            &plan.tree,
            &mut self.retainers.tree_state,
            &self.next_id,
        );

        self.retainers
            .tree_state
            .children_map
            .insert(children_key, root_children);
        self.mark_rows_dirty();
    }

    pub(super) fn drain_results(&mut self, snap: &HeapSnapshot) -> bool {
        let mut changed = false;
        while let Ok(result) = self.result_rx.try_recv() {
            match result {
                WorkResult::ReachableSize { ordinal, size } => {
                    self.reachable_pending.remove(&ordinal);
                    self.reachable_sizes.insert(ordinal, size);
                    // Patch reachable size directly into cached rows instead of
                    // rebuilding the entire flattened tree (which can be expensive
                    // for deeply expanded retainer views).
                    for row in &mut self.cached_rows {
                        if let FlatRowKind::HeapNode {
                            node_ordinal: Some(o),
                            ref mut reachable_size,
                            ..
                        } = row.render.kind
                        {
                            if o == ordinal {
                                *reachable_size = Some(size);
                            }
                        }
                    }
                    changed = true;
                }
                WorkResult::RetainerPlan { request, plan } => {
                    if self.retainers.plan_pending.as_ref() == Some(&request) {
                        match request.kind {
                            RetainerPlanKind::Target => {
                                if self.retainers.target == Some(request.ordinal) {
                                    if plan.reached_gc_roots {
                                        self.apply_retainers_plan(request.ordinal, plan, snap);
                                        self.retainers.plan_message = None;
                                    } else if plan.truncated {
                                        self.retainers.plan_message = Some(format!(
                                            "No GC-root path found within current limits (depth {}, nodes {}).",
                                            RETAINER_AUTO_EXPAND_DEPTH, RETAINER_AUTO_EXPAND_NODES
                                        ));
                                    } else {
                                        self.retainers.plan_message = Some(
                                            "No retainer path to (GC roots) found within current limits."
                                                .to_string(),
                                        );
                                    }
                                    changed = true;
                                }
                            }
                            RetainerPlanKind::Subtree(ck) => {
                                if plan.reached_gc_roots {
                                    self.apply_retainers_subtree_plan(
                                        request.ordinal,
                                        ck,
                                        plan,
                                        snap,
                                    );
                                    self.retainers.plan_message = None;
                                } else if plan.truncated {
                                    self.retainers.plan_message = Some(format!(
                                        "No GC-root path found within current limits (depth {}, nodes {}).",
                                        RETAINER_AUTO_EXPAND_DEPTH, RETAINER_AUTO_EXPAND_NODES
                                    ));
                                } else {
                                    self.retainers.plan_message = Some(
                                        "No retainer path to (GC roots) found within current limits."
                                            .to_string(),
                                    );
                                }
                                changed = true;
                            }
                        }
                        self.retainers.plan_pending = None;
                    }
                }
                WorkResult::ExtensionName { extension_id, name } => {
                    self.extension_pending.remove(&extension_id);
                    if let Some(name) = name {
                        self.extension_names.insert(extension_id, name);
                        self.mark_rows_dirty();
                        changed = true;
                    }
                }
            }
        }
        changed
    }

    pub(super) fn queue_reachable(&mut self, ordinal: NodeOrdinal) {
        if !self.reachable_sizes.contains_key(&ordinal)
            && !self.reachable_pending.contains(&ordinal)
        {
            self.reachable_pending.insert(ordinal);
            let _ = self.work_tx.send(WorkItem::ReachableSize(ordinal));
        }
    }

    pub(super) fn queue_retainer_plan(&mut self, ordinal: NodeOrdinal, kind: RetainerPlanKind) {
        let request = PendingRetainerPlan { ordinal, kind };
        self.retainers.plan_pending = Some(request.clone());
        self.retainers.plan_message = None;
        let _ = self.work_tx.send(WorkItem::RetainerPlan(request));
    }

    pub(super) fn push_history(&mut self, ordinal: NodeOrdinal) {
        if self.history.last() != Some(&ordinal) {
            self.history.push(ordinal);
            self.history_ids.push(mint_id(&self.next_id));
        }
    }

    pub(super) fn set_retainers_target(&mut self, ordinal: NodeOrdinal, snap: &HeapSnapshot) {
        self.push_history(ordinal);
        self.retainers.target = Some(ordinal);
        let root_id = mint_id(&self.next_id);
        self.retainers.root_id = Some(root_id);
        self.retainers.tree_state = TreeState::new();
        self.retainers.gc_root_path_edges.clear();
        self.retainers.unfiltered_nodes.clear();
        self.retainers.plan_message = None;
        let key = ChildrenKey::Retainers(root_id, ordinal);
        let children = compute_children(
            &key,
            root_id,
            snap,
            &self.sorted_aggregates,
            &self.retainers.tree_state.edge_windows,
            &self.retainers.tree_state.class_member_windows,
            &self.retainers.tree_state.edge_filters,
            "",
            None,
            SummaryFilterMode::All,
            &self.next_id,
        );
        self.retainers.tree_state.children_map.insert(key, children);
        self.queue_retainer_plan(ordinal, RetainerPlanKind::Target);
        self.mark_rows_dirty();
        self.current_view = ViewType::Retainers;
    }

    pub(super) fn show_in_summary(&mut self, ordinal: NodeOrdinal, snap: &HeapSnapshot) {
        self.push_history(ordinal);
        // Find which aggregate contains this ordinal
        let agg_idx = self
            .sorted_aggregates
            .iter()
            .position(|agg| agg.node_ordinals.contains(&ordinal));
        let Some(agg_idx) = agg_idx else { return };

        // Switch to Summary view and clear filter
        self.current_view = ViewType::Summary;
        self.summary_filter.clear();

        // Adjust the class member window so the target ordinal is visible.
        let agg = &self.sorted_aggregates[agg_idx];
        if let Some(member_pos) = agg.node_ordinals.iter().position(|o| *o == ordinal) {
            let w = self
                .summary_state
                .class_member_windows
                .get(&agg_idx)
                .copied()
                .unwrap_or_default();
            if member_pos < w.start || member_pos >= w.start + w.count {
                // Re-center the window so the target is in the middle.
                let new_start = member_pos.saturating_sub(w.count / 2);
                self.summary_state.class_member_windows.insert(
                    agg_idx,
                    EdgeWindow {
                        start: new_start,
                        count: w.count,
                    },
                );
                // Invalidate cached children so they get recomputed with the new window.
                let ck = ChildrenKey::ClassMembers(agg_idx);
                self.summary_state.children_map.remove(&ck);
            }
        }

        // Expand the aggregate group
        let group_id = self.summary_ids[agg_idx];
        let ck = ChildrenKey::ClassMembers(agg_idx);
        if !self.summary_state.expanded.contains(&group_id) {
            self.summary_state.expanded.insert(group_id);
        }
        if !self.summary_state.children_map.contains_key(&ck) {
            let children = compute_children(
                &ck,
                group_id,
                snap,
                &self.sorted_aggregates,
                &self.summary_state.edge_windows,
                &self.summary_state.class_member_windows,
                &self.summary_state.edge_filters,
                self.member_filter_for(agg_idx),
                Some(&self.retainers.gc_root_path_edges),
                SummaryFilterMode::All,
                &self.next_id,
            );
            self.summary_state.children_map.insert(ck, children);
        }

        // Rebuild rows and find the target member
        self.rebuild_rows(snap);
        let group_row = self.cached_rows.iter().position(|r| r.nav.id == group_id);
        let target_idx = self
            .cached_rows
            .iter()
            .position(|r| r.node_ordinal() == Some(ordinal) && r.nav.parent_row == group_row);

        if let Some(idx) = target_idx {
            self.summary_state.cursor = idx;
        } else if let Some(idx) = group_row {
            // Fallback: go to the group header
            self.summary_state.cursor = idx;
        }
    }

    pub(super) fn show_in_dominators(&mut self, ordinal: NodeOrdinal, snap: &HeapSnapshot) {
        self.push_history(ordinal);
        let gc_roots = snap.gc_roots_ordinal();

        // Build path from target up to gc_roots via dominator tree.
        let mut path = vec![ordinal];
        let mut cur = ordinal;
        while cur != gc_roots {
            let dom = snap.dominator_of(cur);
            if dom == cur {
                // Self-loop means unreachable from gc_roots in dominator tree.
                return;
            }
            path.push(dom);
            cur = dom;
        }
        path.reverse(); // now: gc_roots → ... → target

        self.current_view = ViewType::Dominators;

        // The gc_roots node is at depth 0 with dominators_root_id.
        // Expand each node along the path.
        let mut parent_id = self.dominators_root_id;
        for i in 0..path.len() - 1 {
            let parent_ord = path[i];
            let child_ord = path[i + 1];
            let ck = ChildrenKey::DominatedChildren(parent_ord);

            // Ensure children are computed.
            if !self.dominators_state.children_map.contains_key(&ck) {
                let children = compute_children(
                    &ck,
                    parent_id,
                    snap,
                    &self.sorted_aggregates,
                    &self.dominators_state.edge_windows,
                    &self.dominators_state.class_member_windows,
                    &self.dominators_state.edge_filters,
                    "",
                    None,
                    SummaryFilterMode::All,
                    &self.next_id,
                );
                self.dominators_state
                    .children_map
                    .insert(ck.clone(), children);
            }

            // Mark parent as expanded.
            self.dominators_state.expanded.insert(parent_id);

            // Find the child's NodeId.
            let child_id = self
                .dominators_state
                .children_map
                .get(&ck)
                .and_then(|children| {
                    children
                        .iter()
                        .find(|c| c.node_ordinal == Some(child_ord))
                        .map(|c| c.id)
                });
            let Some(child_id) = child_id else { return };
            parent_id = child_id;
        }

        // Rebuild rows and position cursor on the target.
        self.rebuild_rows(snap);
        if let Some(idx) = self.cached_rows.iter().position(|r| r.nav.id == parent_id) {
            self.dominators_state.cursor = idx;
        }
    }

    pub(super) fn show_in_containment(&mut self, ordinal: NodeOrdinal, snap: &HeapSnapshot) {
        self.push_history(ordinal);
        let synthetic_root = snap.synthetic_root_ordinal();

        if ordinal == synthetic_root {
            self.current_view = ViewType::Containment;
            return;
        }

        // Build path from target up to synthetic_root by following retainers
        // with the shortest distance.  This is a heuristic — it may not follow
        // the exact edge-tree path, but avoids an expensive BFS from root.
        let mut path = vec![ordinal];
        let mut cur = ordinal;
        let mut seen = rustc_hash::FxHashSet::default();
        seen.insert(cur);
        while cur != synthetic_root {
            let mut best: Option<(NodeOrdinal, usize, Distance)> = None; // (retainer_ord, edge_idx, distance)
            snap.for_each_retainer(cur, |edge_idx, ret_ord| {
                if snap.is_invisible_edge(edge_idx) || seen.contains(&ret_ord) {
                    return;
                }
                let dist = snap.node_distance(ret_ord);
                if best.is_none() || dist < best.unwrap().2 {
                    best = Some((ret_ord, edge_idx, dist));
                }
            });
            let Some((ret_ord, _, _)) = best else { return };
            seen.insert(ret_ord);
            path.push(ret_ord);
            cur = ret_ord;
        }
        path.reverse(); // now: synthetic_root → ... → target

        self.current_view = ViewType::Containment;

        // Expand along the path. The containment root's children are
        // already computed under Edges(synthetic_root).
        let mut parent_id = self.containment_root_id;
        for i in 0..path.len() - 1 {
            let parent_ord = path[i];
            let child_ord = path[i + 1];
            let ck = ChildrenKey::Edges(parent_id, parent_ord);

            // Find the child's 0-based position in the rendered edge order.
            // We must use compute_edges (with a full window) so that sorting
            // for NativeContext/JSGlobal* parents and edge-filter label
            // matching use exactly the same logic as the real render path.
            let filter = self
                .containment_state
                .edge_filters
                .get(&parent_ord)
                .map(|s| s.as_str())
                .unwrap_or("");
            let dummy_id = std::cell::Cell::new(u64::MAX / 2);
            let full = compute_edges(
                snap,
                parent_ord,
                EdgeWindow {
                    start: 0,
                    count: usize::MAX,
                },
                filter,
                &dummy_id,
            );
            let child_pos = full.iter().position(|c| c.node_ordinal == Some(child_ord));
            let Some(child_pos) = child_pos else { return };

            let w = self
                .containment_state
                .edge_windows
                .get(&parent_id)
                .copied()
                .unwrap_or_default();
            let needs_recompute = if child_pos < w.start || child_pos >= w.start + w.count {
                // Re-center the window so the target is in the middle.
                let new_start = child_pos.saturating_sub(w.count / 2);
                self.containment_state.edge_windows.insert(
                    parent_id,
                    EdgeWindow {
                        start: new_start,
                        count: w.count,
                    },
                );
                true
            } else {
                !self.containment_state.children_map.contains_key(&ck)
            };

            if needs_recompute {
                let children = compute_children(
                    &ck,
                    parent_id,
                    snap,
                    &self.sorted_aggregates,
                    &self.containment_state.edge_windows,
                    &self.containment_state.class_member_windows,
                    &self.containment_state.edge_filters,
                    "",
                    None,
                    SummaryFilterMode::All,
                    &self.next_id,
                );
                self.containment_state
                    .children_map
                    .insert(ck.clone(), children);
            }

            // Mark parent as expanded.
            self.containment_state.expanded.insert(parent_id);

            // Find the child's NodeId.
            let child_id = self
                .containment_state
                .children_map
                .get(&ck)
                .and_then(|children| {
                    children
                        .iter()
                        .find(|c| c.node_ordinal == Some(child_ord))
                        .map(|c| c.id)
                });
            let Some(child_id) = child_id else { return };
            parent_id = child_id;
        }

        // Rebuild rows and position cursor on the target.
        self.rebuild_rows(snap);
        if let Some(idx) = self.cached_rows.iter().position(|r| r.nav.id == parent_id) {
            self.containment_state.cursor = idx;
        }
    }

    pub(super) fn expand(
        &mut self,
        id: NodeId,
        children_key: Option<ChildrenKey>,
        snap: &HeapSnapshot,
    ) {
        if let Some(ck) = children_key {
            let manual_retainer_expand = self.current_view == ViewType::Retainers
                && matches!(ck, ChildrenKey::Retainers(..));
            if let ChildrenKey::Retainers(nid, _) = ck {
                if manual_retainer_expand {
                    self.retainers.unfiltered_nodes.insert(nid);
                }
            }
            // Compute children before taking mutable borrow on tree state.
            // In Retainers, explicit user expansion should show the full subtree even if
            // the background GC-root plan previously cached a filtered subtree here.
            let needs_compute =
                manual_retainer_expand || !self.current_tree_state().children_map.contains_key(&ck);
            let children = if needs_compute {
                match &ck {
                    ChildrenKey::DiffMembers(i) => Some(self.compute_diff_members(*i, snap)),
                    ChildrenKey::CompareEdges(_, ord) => {
                        let cs = &self.diff.compare_snapshots[self.diff.current_idx];
                        let state = self.current_tree_state();
                        let w = state.edge_windows.get(&id).copied().unwrap_or_default();
                        let filter = state
                            .edge_filters
                            .get(ord)
                            .map(|s| s.as_str())
                            .unwrap_or("");
                        Some(compute_compare_edges(cs, *ord, w, filter, &self.next_id))
                    }
                    _ => {
                        let state = self.current_tree_state();
                        let filter = if let ChildrenKey::ClassMembers(i) = ck {
                            self.member_filter_for(i)
                        } else {
                            ""
                        };
                        let unreachable_filter = if self.current_view == ViewType::Summary {
                            SummaryFilterMode::All
                        } else {
                            SummaryFilterMode::All
                        };
                        Some(compute_children(
                            &ck,
                            id,
                            snap,
                            &self.sorted_aggregates,
                            &state.edge_windows,
                            &state.class_member_windows,
                            &state.edge_filters,
                            filter,
                            match ck {
                                ChildrenKey::Retainers(nid, _) => self.retainer_path_filter(nid),
                                _ => {
                                    if manual_retainer_expand {
                                        None
                                    } else {
                                        Some(&self.retainers.gc_root_path_edges)
                                    }
                                }
                            },
                            unreachable_filter,
                            &self.next_id,
                        ))
                    }
                }
            } else {
                None
            };
            let state = self.current_tree_state_mut();
            state.expanded.insert(id);
            if let Some(children) = children {
                state.children_map.insert(ck, children);
            }
            self.mark_rows_dirty();
        }
    }

    pub(super) fn collapse(&mut self, id: NodeId) {
        // Only remove the collapsed node from the expanded set.
        // Children and descendants stay cached so re-expanding instantly
        // restores the previous subtree structure.
        let state = self.current_tree_state_mut();
        state.expanded.remove(&id);
        self.mark_rows_dirty();
    }

    // Find the expanded paged-children parent of the cursor row.
    pub(super) fn find_paged_children_parent(
        &self,
        snap: &HeapSnapshot,
    ) -> Option<PagedChildrenParent> {
        let row = self.current_row()?;

        match &row.nav.children_key {
            Some(ChildrenKey::Edges(_, ord)) if row.nav.is_expanded => {
                return Some(PagedChildrenParent::Edges {
                    id: row.nav.id,
                    ordinal: *ord,
                    is_compare: false,
                });
            }
            Some(ChildrenKey::CompareEdges(_, ord)) if row.nav.is_expanded => {
                return Some(PagedChildrenParent::Edges {
                    id: row.nav.id,
                    ordinal: *ord,
                    is_compare: true,
                });
            }
            Some(ChildrenKey::Retainers(id, ord)) if row.nav.is_expanded => {
                return Some(PagedChildrenParent::Retainers {
                    id: *id,
                    ordinal: *ord,
                });
            }
            Some(ChildrenKey::ClassMembers(i)) if row.nav.is_expanded => {
                return Some(PagedChildrenParent::ClassMembers { agg_idx: *i });
            }
            _ => {}
        }
        if let Some(parent_row) = row.nav.parent_row {
            return self
                .cached_rows
                .get(parent_row)
                .and_then(|r| match &r.nav.children_key {
                    Some(ChildrenKey::Edges(_, ord)) => Some(PagedChildrenParent::Edges {
                        id: r.nav.id,
                        ordinal: *ord,
                        is_compare: false,
                    }),
                    Some(ChildrenKey::CompareEdges(_, ord)) => Some(PagedChildrenParent::Edges {
                        id: r.nav.id,
                        ordinal: *ord,
                        is_compare: true,
                    }),
                    Some(ChildrenKey::Retainers(id, ord)) => Some(PagedChildrenParent::Retainers {
                        id: *id,
                        ordinal: *ord,
                    }),
                    Some(ChildrenKey::ClassMembers(i)) => {
                        Some(PagedChildrenParent::ClassMembers { agg_idx: *i })
                    }
                    _ => None,
                });
        }
        // Depth-0 rows in containment/retainers have no ancestor row;
        // the implicit parent is the view's root node.
        match self.current_view {
            ViewType::Containment => Some(PagedChildrenParent::Edges {
                id: self.containment_root_id,
                ordinal: snap.synthetic_root_ordinal(),
                is_compare: false,
            }),
            ViewType::Retainers => self
                .retainers
                .target
                .zip(self.retainers.root_id)
                .map(|(ordinal, id)| PagedChildrenParent::Retainers { id, ordinal }),
            _ => None,
        }
    }

    /// Was the cursor on a paging status line before the current operation?
    fn cursor_on_status_line(&self) -> bool {
        self.current_row().is_some_and(|r| {
            !r.nav.has_children
                && match &r.render.kind {
                    FlatRowKind::HeapNode { node_ordinal, .. } => node_ordinal.is_none(),
                    _ => false,
                }
        })
    }

    // Re-flatten the tree and clamp the cursor so it doesn't go past the
    // last child row of the given paged-children parent.  When
    // `snap_to_status_line` is true the cursor is moved to the last child
    // (the status line) instead of merely clamped.
    pub(super) fn clamp_cursor_to_paged_parent(
        &mut self,
        parent: PagedChildrenParent,
        snap: &HeapSnapshot,
        snap_to_status_line: bool,
    ) {
        self.rebuild_rows(snap);
        let target_key = match parent {
            PagedChildrenParent::Edges {
                id,
                ordinal,
                is_compare,
            } => {
                if is_compare {
                    ChildrenKey::CompareEdges(id, ordinal)
                } else {
                    ChildrenKey::Edges(id, ordinal)
                }
            }
            PagedChildrenParent::Retainers { id, ordinal } => ChildrenKey::Retainers(id, ordinal),
            PagedChildrenParent::ClassMembers { agg_idx } => ChildrenKey::ClassMembers(agg_idx),
        };
        let parent_idx = self.cached_rows.iter().position(|r| {
            matches!(&r.nav.children_key, Some(ck) if *ck == target_key) && r.nav.is_expanded
        });
        if let Some(pidx) = parent_idx {
            let last_child = self.subtree_end_index(pidx);
            let state = self.current_tree_state_mut();
            if snap_to_status_line {
                state.cursor = last_child;
            } else if state.cursor > last_child {
                state.cursor = last_child;
            }
        }
    }

    pub(super) fn adjust_edge_count(&mut self, delta: isize, snap: &HeapSnapshot) {
        let Some(parent) = self.find_paged_children_parent(snap) else {
            return;
        };
        let was_on_status = self.cursor_on_status_line();
        if let PagedChildrenParent::ClassMembers { agg_idx } = parent {
            let state = self.current_tree_state_mut();
            let mut w = state
                .class_member_windows
                .get(&agg_idx)
                .copied()
                .unwrap_or_default();
            w.count = (w.count as isize + delta).max(EDGE_PAGE_SIZE as isize) as usize;
            state.class_member_windows.insert(agg_idx, w);
            self.recompute_paged_children(parent.clone(), snap);
            self.clamp_cursor_to_paged_parent(parent, snap, was_on_status);
            return;
        }
        let parent_id = match &parent {
            PagedChildrenParent::Edges { id, .. } | PagedChildrenParent::Retainers { id, .. } => {
                *id
            }
            PagedChildrenParent::ClassMembers { .. } => unreachable!(),
        };

        let state = self.current_tree_state_mut();
        let mut w = state
            .edge_windows
            .get(&parent_id)
            .copied()
            .unwrap_or_default();
        w.count = (w.count as isize + delta).max(EDGE_PAGE_SIZE as isize) as usize;
        state.edge_windows.insert(parent_id, w);

        self.recompute_paged_children(parent.clone(), snap);
        self.clamp_cursor_to_paged_parent(parent, snap, was_on_status);
    }

    /// Shift the paged-children window forward (`direction` = 1) or backward
    /// (`direction` = -1) by the window's current page size.
    pub(super) fn shift_edge_window(&mut self, direction: isize, snap: &HeapSnapshot) {
        let Some(parent) = self.find_paged_children_parent(snap) else {
            return;
        };
        let was_on_status = self.cursor_on_status_line();
        if let PagedChildrenParent::ClassMembers { agg_idx } = parent {
            let agg = &self.sorted_aggregates[agg_idx];
            let filter = self.member_filter_for(agg_idx);
            let total = if !filter.is_empty() {
                agg.node_ordinals
                    .iter()
                    .filter(|ord| contains_ignore_case(snap.node_raw_name(**ord), filter))
                    .count()
            } else {
                agg.node_ordinals.len()
            };
            let state = &mut self.summary_state;
            let mut w = state
                .class_member_windows
                .get(&agg_idx)
                .copied()
                .unwrap_or_default();
            let step = direction * w.count as isize;
            w.start = shifted_window_start(w.start, w.count, total, step);
            state.class_member_windows.insert(agg_idx, w);
            self.recompute_paged_children(parent.clone(), snap);
            self.clamp_cursor_to_paged_parent(parent, snap, was_on_status);
            return;
        }
        let (parent_id, ord) = match &parent {
            PagedChildrenParent::Edges { id, ordinal, .. }
            | PagedChildrenParent::Retainers { id, ordinal, .. } => (*id, *ordinal),
            PagedChildrenParent::ClassMembers { .. } => unreachable!(),
        };

        let total = match &parent {
            PagedChildrenParent::Edges { is_compare, .. } => {
                let is_compare = *is_compare;
                let the_snap = if is_compare {
                    &self.diff.compare_snapshots[self.diff.current_idx]
                } else {
                    snap
                };

                let filter = self
                    .current_tree_state()
                    .edge_filters
                    .get(&ord)
                    .cloned()
                    .unwrap_or_default();
                the_snap
                    .iter_edges(ord)
                    .filter(|&(ei, _)| !the_snap.is_invisible_edge(ei))
                    .filter(|&(edge_idx, child_ord)| {
                        if filter.is_empty() {
                            return true;
                        }
                        let label = children::format_edge_label(the_snap, edge_idx, child_ord);
                        contains_ignore_case(&label, &filter)
                    })
                    .count()
            }
            PagedChildrenParent::Retainers { id, .. } => {
                let path_edges = self.retainer_path_filter(*id);
                let mut total = 0usize;
                snap.for_each_retainer(ord, |edge_idx, _ret_ord| {
                    if let Some(pe) = path_edges {
                        let is_weak = snap.edge_type_name(edge_idx) == "weak";
                        if !is_weak && !pe.contains(&edge_idx) {
                            return;
                        }
                    }
                    total += 1;
                });
                total
            }
            PagedChildrenParent::ClassMembers { .. } => unreachable!(),
        };

        let state = self.current_tree_state_mut();
        let mut w = state
            .edge_windows
            .get(&parent_id)
            .copied()
            .unwrap_or_default();
        let step = direction * w.count as isize;
        w.start = shifted_window_start(w.start, w.count, total, step);
        state.edge_windows.insert(parent_id, w);

        self.recompute_paged_children(parent.clone(), snap);
        self.clamp_cursor_to_paged_parent(parent, snap, was_on_status);
    }

    pub(super) fn recompute_paged_children(
        &mut self,
        parent: PagedChildrenParent,
        snap: &HeapSnapshot,
    ) {
        let (ck, children) = match parent {
            PagedChildrenParent::ClassMembers { agg_idx } => {
                let ck = ChildrenKey::ClassMembers(agg_idx);
                let w = self
                    .summary_state
                    .class_member_windows
                    .get(&agg_idx)
                    .copied()
                    .unwrap_or_default();
                let children = compute_class_members(
                    snap,
                    &self.sorted_aggregates[agg_idx],
                    w,
                    self.member_filter_for(agg_idx),
                    SummaryFilterMode::All,
                    &self.next_id,
                );
                (ck, children)
            }
            PagedChildrenParent::Edges {
                id,
                ordinal: ord,
                is_compare,
            } => {
                let ck = if is_compare {
                    ChildrenKey::CompareEdges(id, ord)
                } else {
                    ChildrenKey::Edges(id, ord)
                };
                let w = self
                    .current_tree_state()
                    .edge_windows
                    .get(&id)
                    .copied()
                    .unwrap_or_default();
                let filter = self
                    .current_tree_state()
                    .edge_filters
                    .get(&ord)
                    .cloned()
                    .unwrap_or_default();
                let the_snap = if is_compare {
                    &self.diff.compare_snapshots[self.diff.current_idx]
                } else {
                    snap
                };
                let children = if is_compare {
                    compute_compare_edges(the_snap, ord, w, &filter, &self.next_id)
                } else {
                    compute_edges(the_snap, ord, w, &filter, &self.next_id)
                };
                (ck, children)
            }
            PagedChildrenParent::Retainers { id, ordinal: ord } => {
                let ck = ChildrenKey::Retainers(id, ord);
                let w = self
                    .current_tree_state()
                    .edge_windows
                    .get(&id)
                    .copied()
                    .unwrap_or_default();
                let children =
                    compute_retainers(snap, ord, w, self.retainer_path_filter(id), &self.next_id);
                (ck, children)
            }
        };
        self.current_tree_state_mut()
            .children_map
            .insert(ck, children);
        self.mark_rows_dirty();
    }

    pub(super) fn switch_diff(&mut self, idx: usize) {
        if idx >= self.diff.all_diffs.len() {
            return;
        }
        self.diff.current_idx = idx;
        let (diffs, ids) = self.diff.all_diffs[idx].clone();
        self.diff.sorted_diffs = diffs;
        self.diff.diff_ids = ids;
        self.diff.tree_state = TreeState::new();
        self.diff.filter.clear();
        self.mark_rows_dirty();
    }
}
