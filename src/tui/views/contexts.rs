use crate::snapshot::HeapSnapshot;

use super::super::App;
use super::super::types::*;

impl App {
    pub(in crate::tui) fn flatten_contexts(
        &self,
        state: &TreeState,
        rows: &mut Vec<FlatRow>,
        snap: &HeapSnapshot,
    ) {
        for (ctx, &id) in snap.native_contexts().iter().zip(self.contexts_ids.iter()) {
            let ord = ctx.ordinal;
            let has_children = snap.node_edge_count(ord) > 0;
            let mut label = snap.native_context_label(ord);
            // Replace chrome-extension:// URLs with resolved extension names
            if let Some(url) = snap.native_context_url(ord) {
                if let Some(ext_id) = url
                    .strip_prefix("chrome-extension://")
                    .and_then(|s| s.split('/').next())
                {
                    if let Some(name) = self.extension_names.get(ext_id) {
                        label = label.replace(url, &format!("{name} ({ext_id})"));
                    }
                }
            }
            self.push_heap_node(
                rows,
                state,
                snap,
                id,
                ord,
                label.into(),
                if has_children {
                    Some(ChildrenKey::Edges(id, ord))
                } else {
                    None
                },
                snap.native_context_detachedness(ord),
            );
        }
    }
}
