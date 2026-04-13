use super::*;
use crate::print::retainers::{RetainerAutoExpandLimits, plan_gc_root_retainer_paths};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub struct BenchApp {
    inner: App,
}

impl BenchApp {
    pub fn new(snap: &HeapSnapshot) -> Self {
        let (work_tx, _work_rx) = std::sync::mpsc::channel();
        let (_result_tx, result_rx) = std::sync::mpsc::channel();
        Self {
            inner: {
                let mut sorted: Vec<_> = snap.aggregates_with_filter();
                sorted.sort_by(|a, b| {
                    b.max_ret
                        .partial_cmp(&a.max_ret)
                        .unwrap()
                        .then(a.first_seen.cmp(&b.first_seen))
                });
                App::new_with_aggregates(snap, sorted, Vec::new(), work_tx, result_rx)
            },
        }
    }

    /// Set retainers target and synchronously compute + apply the GC-root
    /// retainer plan (normally done on a background thread).
    pub fn set_view_retainers_with_plan(&mut self, ordinal: usize, snap: &HeapSnapshot) {
        let ord = crate::types::NodeOrdinal(ordinal);
        self.inner.set_retainers_target(ord, snap);
        let plan = plan_gc_root_retainer_paths(
            snap,
            ord,
            RetainerAutoExpandLimits {
                max_depth: RETAINER_AUTO_EXPAND_DEPTH,
                max_nodes: RETAINER_AUTO_EXPAND_NODES,
            },
        );
        if plan.reached_gc_roots {
            self.inner.apply_retainers_plan(ord, plan, snap);
        }
        self.inner.rebuild_rows(snap);
    }

    pub fn rebuild_rows(&mut self, snap: &HeapSnapshot) {
        self.inner.rebuild_rows(snap);
    }

    pub fn row_count(&self) -> usize {
        self.inner.cached_rows.len()
    }

    pub fn set_view_summary(&mut self, snap: &HeapSnapshot) {
        self.inner.set_view(ViewType::Summary, snap);
    }

    pub fn set_view_containment(&mut self, snap: &HeapSnapshot) {
        self.inner.set_view(ViewType::Containment, snap);
    }

    pub fn set_view_dominators(&mut self, snap: &HeapSnapshot) {
        self.inner.set_view(ViewType::Dominators, snap);
    }

    pub fn set_view_retainers(&mut self, ordinal: usize, snap: &HeapSnapshot) {
        self.inner
            .set_retainers_target(crate::types::NodeOrdinal(ordinal), snap);
    }

    pub fn key(&mut self, c: char, snap: &HeapSnapshot) {
        self.inner
            .handle_normal_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), snap);
    }

    pub fn key_enter(&mut self, snap: &HeapSnapshot) {
        self.inner
            .handle_normal_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), snap);
    }

    pub fn key_down(&mut self, snap: &HeapSnapshot) {
        self.inner
            .handle_normal_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), snap);
    }

    pub fn key_up(&mut self, snap: &HeapSnapshot) {
        self.inner
            .handle_normal_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), snap);
    }

    pub fn set_summary_filter(&mut self, filter: &str) {
        self.inner.summary_filter = filter.to_lowercase();
        self.inner.rows_dirty = true;
    }

    /// Expand the row at the current cursor position.
    pub fn expand_at_cursor(&mut self, snap: &HeapSnapshot) {
        if let Some(row) = self
            .inner
            .cached_rows
            .get(self.inner.current_tree_state().cursor)
        {
            let id = row.nav.id;
            let ck = row.nav.children_key.clone();
            self.inner.expand(id, ck, snap);
        }
    }

    pub fn set_cursor(&mut self, pos: usize) {
        self.inner.current_tree_state_mut().cursor = pos;
    }

    pub fn cursor(&self) -> usize {
        self.inner.current_tree_state().cursor
    }
}
