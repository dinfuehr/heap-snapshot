use crate::snapshot::HeapSnapshot;

use super::super::App;
use super::super::types::*;

impl App {
    pub(in crate::tui) fn flatten_history(
        &self,
        state: &TreeState,
        rows: &mut Vec<FlatRow>,
        snap: &HeapSnapshot,
    ) {
        // Flat list: most recent first, expandable to show outgoing refs
        for (&ord, &id) in self.history.iter().zip(self.history_ids.iter()).rev() {
            let has_children = snap.node_edge_count(ord) > 0;
            let name = snap.node_display_name(ord);
            let node_id = snap.node_id(ord);
            self.push_heap_node(
                rows,
                state,
                snap,
                id,
                ord,
                format!("{name} @{node_id}").into(),
                if has_children {
                    Some(ChildrenKey::Edges(id, ord))
                } else {
                    None
                },
                snap.node_detachedness(ord),
            );
        }
    }
}
