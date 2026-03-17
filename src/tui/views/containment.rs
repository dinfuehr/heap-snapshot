use crate::snapshot::HeapSnapshot;

use super::super::App;
use super::super::types::*;

impl App {
    pub(in crate::tui) fn flatten_containment(
        &self,
        state: &TreeState,
        rows: &mut Vec<FlatRow>,
        snap: &HeapSnapshot,
    ) {
        let root_key = ChildrenKey::Edges(snap.synthetic_root_ordinal());
        self.flatten_children(&root_key, None, 0, state, rows, snap);
    }
}
