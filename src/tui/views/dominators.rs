use crate::snapshot::HeapSnapshot;

use super::super::App;
use super::super::types::*;

impl App {
    pub(in crate::tui) fn flatten_dominators(
        &self,
        state: &TreeState,
        rows: &mut Vec<FlatRow>,
        snap: &HeapSnapshot,
    ) {
        let root_ord = snap.gc_roots_ordinal();
        let has_children = !snap.get_dominated_children(root_ord).is_empty();
        let name = snap.node_display_name(root_ord);
        let node_id = snap.node_id(root_ord);
        self.push_heap_node(
            rows,
            state,
            snap,
            self.dominators_root_id,
            root_ord,
            format!("{name} @{node_id}").into(),
            if has_children {
                Some(ChildrenKey::DominatedChildren(root_ord))
            } else {
                None
            },
            snap.node_detachedness(root_ord),
        );
    }
}
