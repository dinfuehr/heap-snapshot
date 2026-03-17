use crate::snapshot::HeapSnapshot;
use crate::types::NodeOrdinal;

use super::super::App;
use super::super::types::*;

impl App {
    pub(in crate::tui) fn flatten_contexts(
        &self,
        state: &TreeState,
        rows: &mut Vec<FlatRow>,
        snap: &HeapSnapshot,
    ) {
        for (&ctx_ord, &id) in snap.native_contexts().iter().zip(self.contexts_ids.iter()) {
            let ord = NodeOrdinal(ctx_ord);
            let has_children = snap.node_edge_count(ord) > 0;
            self.push_heap_node(
                rows,
                state,
                snap,
                id,
                ord,
                snap.native_context_label(ord).into(),
                if has_children {
                    Some(ChildrenKey::Edges(ord))
                } else {
                    None
                },
                snap.native_context_detachedness(ord),
            );
        }
    }
}
