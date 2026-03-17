use crate::snapshot::HeapSnapshot;

use super::super::App;
use super::super::types::*;

impl App {
    pub(in crate::tui) fn flatten_retainers(
        &self,
        state: &TreeState,
        rows: &mut Vec<FlatRow>,
        snap: &HeapSnapshot,
    ) {
        if let (Some(target), Some(root_id)) = (self.retainers.target, self.retainers.root_id) {
            let root_key = ChildrenKey::Retainers(root_id, target);
            self.flatten_children(&root_key, None, 0, state, rows, snap);
        }
    }
}
