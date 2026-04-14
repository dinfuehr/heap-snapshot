use std::rc::Rc;

use crate::snapshot::HeapSnapshot;
use crate::types::NodeOrdinal;

use super::App;
use super::types::*;

impl App {
    pub(super) fn flatten_tree(&self, snap: &HeapSnapshot) -> Vec<FlatRow> {
        let mut rows = Vec::new();

        match self.current_view {
            ViewType::Help | ViewType::Statistics | ViewType::Timeline => {
                return rows;
            }
            _ => {}
        }

        let state = self.current_tree_state();
        match self.current_view {
            ViewType::Summary => self.flatten_summary(state, &mut rows, snap),
            ViewType::Containment => self.flatten_containment(state, &mut rows, snap),
            ViewType::Dominators => self.flatten_dominators(state, &mut rows, snap),
            ViewType::Retainers => self.flatten_retainers(state, &mut rows, snap),
            ViewType::Contexts => self.flatten_contexts(state, &mut rows, snap),
            ViewType::History => self.flatten_history(state, &mut rows, snap),
            ViewType::Diff => self.flatten_diff(state, &mut rows, snap),
            ViewType::Help | ViewType::Statistics | ViewType::Timeline => unreachable!(),
        }

        rows
    }

    pub(super) fn flatten_children(
        &self,
        children_key: &ChildrenKey,
        parent_row: Option<usize>,
        depth: usize,
        state: &TreeState,
        rows: &mut Vec<FlatRow>,
        snap: &HeapSnapshot,
    ) {
        struct Frame {
            key: ChildrenKey,
            parent_row: Option<usize>,
            depth: usize,
            child_idx: usize,
        }

        let mut stack = vec![Frame {
            key: children_key.clone(),
            parent_row,
            depth,
            child_idx: 0,
        }];

        while let Some(frame) = stack.last_mut() {
            let Some(children) = state.children_map.get(&frame.key) else {
                stack.pop();
                continue;
            };
            if frame.child_idx >= children.len() {
                stack.pop();
                continue;
            }
            let child = &children[frame.child_idx];
            frame.child_idx += 1;

            let is_expanded = state.expanded.contains(&child.id);
            let cur_depth = frame.depth;
            let parent_row = frame.parent_row;

            rows.push(FlatRow {
                nav: FlatRowNav {
                    id: child.id,
                    parent_row,
                    depth: cur_depth,
                    has_children: child.has_children,
                    is_expanded,
                    children_key: child.children_key.clone(),
                },
                render: FlatRowRender {
                    label: child.label.clone(),
                    kind: FlatRowKind::HeapNode {
                        node_ordinal: child.node_ordinal,
                        distance: child.distance,
                        shallow_size: child.shallow_size,
                        retained_size: child.retained_size,
                        reachable_size: child
                            .node_ordinal
                            .and_then(|o| self.reachable_sizes.get(&o).copied()),
                        detachedness: child.node_ordinal.map(|o| snap.node_detachedness(o)),
                    },
                    is_weak: child.is_weak,
                    is_root_holder: child.is_root_holder,
                },
            });
            let child_row = rows.len() - 1;

            if is_expanded {
                if let Some(ref ck) = child.children_key {
                    stack.push(Frame {
                        key: ck.clone(),
                        parent_row: Some(child_row),
                        depth: cur_depth + 1,
                        child_idx: 0,
                    });
                }
            }
        }
    }

    /// Push a top-level HeapNode row and, if expanded, flatten its children.
    ///
    /// This is the common pattern used by Dominators, Contexts, and History.
    /// `children_key` being `Some` means the node is expandable; `None` means leaf.
    pub(super) fn push_heap_node(
        &self,
        rows: &mut Vec<FlatRow>,
        state: &TreeState,
        snap: &HeapSnapshot,
        id: NodeId,
        ordinal: NodeOrdinal,
        label: Rc<str>,
        children_key: Option<ChildrenKey>,
        detachedness: u8,
    ) {
        let has_children = children_key.is_some();
        let is_expanded = state.expanded.contains(&id);
        rows.push(FlatRow {
            nav: FlatRowNav {
                id,
                parent_row: None,
                depth: 0,
                has_children,
                is_expanded,
                children_key: children_key.clone(),
            },
            render: FlatRowRender {
                label,
                kind: FlatRowKind::HeapNode {
                    node_ordinal: Some(ordinal),
                    distance: Some(snap.node_distance(ordinal)),
                    shallow_size: snap.node_self_size(ordinal) as u64,
                    retained_size: snap.node_retained_size(ordinal),
                    reachable_size: self.reachable_sizes.get(&ordinal).copied(),
                    detachedness: Some(detachedness),
                },
                is_weak: false,
                is_root_holder: false,
            },
        });
        if is_expanded {
            if let Some(ref ck) = children_key {
                let parent_row = rows.len() - 1;
                self.flatten_children(ck, Some(parent_row), 1, state, rows, snap);
            }
        }
    }
}
