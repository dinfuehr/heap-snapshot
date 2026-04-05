use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::snapshot::HeapSnapshot;

use super::types::*;
use super::{App, EDGE_PAGE_SIZE, HORIZONTAL_SCROLL_STEP};

impl App {
    pub(super) fn handle_key(&mut self, key: KeyEvent, snap: &HeapSnapshot) -> bool {
        let prev_view = self.current_view;
        let should_quit = match self.input_mode {
            InputMode::Search => self.handle_search_key(key, snap),
            InputMode::EdgeFilter => self.handle_edge_filter_key(key, snap),
            InputMode::Normal => self.handle_normal_key(key, snap),
        };
        if self.current_view != prev_view {
            self.mark_rows_dirty();
        }
        should_quit
    }

    pub(super) fn handle_search_key(&mut self, key: KeyEvent, snap: &HeapSnapshot) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.search_input.clear();
                self.search_error = None;
            }
            KeyCode::Enter => {
                let input = self.search_input.trim().to_string();
                self.input_mode = InputMode::Normal;
                self.search_input.clear();

                if input.is_empty() {
                    // Empty search clears the filter
                    if self.current_view == ViewType::Diff {
                        self.diff.filter.clear();
                        self.diff.tree_state = TreeState::new();
                    } else {
                        self.summary_filter.clear();
                        self.summary_state = TreeState::new();
                        self.current_view = ViewType::Summary;
                    }
                    self.search_error = None;
                    self.mark_rows_dirty();
                } else if input.starts_with('@') {
                    // @id → show in current view or open retainers
                    let id_str = &input[1..];
                    match id_str.parse::<u64>() {
                        Ok(id) => {
                            match snap.node_for_snapshot_object_id(crate::types::NodeId(id)) {
                                Some(ordinal) => {
                                    if self.current_view == ViewType::Summary {
                                        self.show_in_summary(ordinal, snap);
                                    } else {
                                        self.set_retainers_target(ordinal, snap);
                                    }
                                    self.search_error = None;
                                }
                                None => {
                                    self.search_error = Some(format!("No node with id @{id}"));
                                }
                            }
                        }
                        Err(_) => {
                            self.search_error = Some(format!("Invalid ID: {input}"));
                        }
                    }
                } else if self.current_view == ViewType::Diff {
                    // Text → filter constructor names in Diff view
                    self.diff.filter = input.to_lowercase();
                    self.diff.tree_state = TreeState::new();
                    self.search_error = None;
                    self.mark_rows_dirty();
                } else {
                    // Text → filter constructors in Summary view
                    self.summary_filter = input.to_lowercase();
                    self.summary_state = TreeState::new();
                    self.search_error = None;
                    self.current_view = ViewType::Summary;
                    self.mark_rows_dirty();
                }
            }
            KeyCode::Backspace => {
                self.search_input.pop();
            }
            KeyCode::Char(c) => {
                self.search_input.push(c);
            }
            _ => {}
        }
        false
    }

    pub(super) fn handle_edge_filter_key(&mut self, key: KeyEvent, snap: &HeapSnapshot) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.edge_filter_target = None;
                self.edge_filter_input.clear();
            }
            KeyCode::Enter => {
                let input = self.edge_filter_input.trim().to_lowercase();
                if let (Some(ord), Some(nid)) = (self.edge_filter_target, self.edge_filter_node_id)
                {
                    let is_compare = self.edge_filter_is_compare;
                    let state = self.current_tree_state_mut();
                    if input.is_empty() {
                        state.edge_filters.remove(&ord);
                    } else {
                        state.edge_filters.insert(ord, input);
                    }
                    // Reset window when filter changes
                    state.edge_windows.remove(&nid);

                    let parent = PagedChildrenParent::Edges {
                        id: nid,
                        ordinal: ord,
                        is_compare,
                    };
                    self.recompute_paged_children(parent.clone(), snap);
                    self.clamp_cursor_to_paged_parent(parent, snap, false);
                }
                self.input_mode = InputMode::Normal;
                self.edge_filter_target = None;
                self.edge_filter_node_id = None;
                self.edge_filter_input.clear();
            }
            KeyCode::Backspace => {
                self.edge_filter_input.pop();
            }
            KeyCode::Char(c) => {
                self.edge_filter_input.push(c);
            }
            _ => {}
        }
        false
    }

    pub(super) fn handle_normal_key(&mut self, key: KeyEvent, snap: &HeapSnapshot) -> bool {
        let row_count = self.cached_rows.len();
        let is_scroll_view = self.is_scroll_view();

        if self.handle_page_key(key, row_count) {
            return false;
        }

        match key.code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('1') => self.set_view(ViewType::Summary, snap),
            KeyCode::Char('2') => self.set_view(ViewType::Containment, snap),
            KeyCode::Char('3') => self.set_view(ViewType::Dominators, snap),
            KeyCode::Char('4') => self.set_view(ViewType::Retainers, snap),
            KeyCode::Char('5') if self.diff.has_diff => self.set_view(ViewType::Diff, snap),
            KeyCode::Char('6') => self.set_view(ViewType::Contexts, snap),
            KeyCode::Char('7') => self.set_view(ViewType::History, snap),
            KeyCode::Char('8') => self.set_view(ViewType::Statistics, snap),
            KeyCode::Char('9') => self.set_view(ViewType::Timeline, snap),
            KeyCode::Char('?') => self.set_view(ViewType::Help, snap),
            KeyCode::BackTab => {
                let next_view = match (self.current_view, self.diff.has_diff) {
                    (ViewType::Summary, _) => ViewType::Help,
                    (ViewType::Help, _) => ViewType::Timeline,
                    (ViewType::Timeline, _) => ViewType::Statistics,
                    (ViewType::Statistics, _) => ViewType::History,
                    (ViewType::History, _) => ViewType::Contexts,
                    (ViewType::Contexts, true) => ViewType::Diff,
                    (ViewType::Contexts, false) => ViewType::Retainers,
                    (ViewType::Containment, _) => ViewType::Summary,
                    (ViewType::Dominators, _) => ViewType::Containment,
                    (ViewType::Retainers, _) => ViewType::Dominators,
                    (ViewType::Diff, _) => ViewType::Retainers,
                };
                self.set_view(next_view, snap);
            }
            KeyCode::Tab => {
                let next_view = match (self.current_view, self.diff.has_diff) {
                    (ViewType::Summary, _) => ViewType::Containment,
                    (ViewType::Containment, _) => ViewType::Dominators,
                    (ViewType::Dominators, _) => ViewType::Retainers,
                    (ViewType::Retainers, true) => ViewType::Diff,
                    (ViewType::Retainers, false) => ViewType::Contexts,
                    (ViewType::Diff, _) => ViewType::Contexts,
                    (ViewType::Contexts, _) => ViewType::History,
                    (ViewType::History, _) => ViewType::Statistics,
                    (ViewType::Statistics, _) => ViewType::Timeline,
                    (ViewType::Timeline, _) => ViewType::Help,
                    (ViewType::Help, _) => ViewType::Summary,
                };
                self.set_view(next_view, snap);
            }
            KeyCode::Char('/') => {
                self.input_mode = InputMode::Search;
                self.search_input.clear();
                self.search_error = None;
            }
            KeyCode::Up => {
                if is_scroll_view {
                    self.current_scroll_state_mut().scroll_offset =
                        self.current_scroll_state().scroll_offset.saturating_sub(1);
                } else {
                    self.current_tree_state_mut().cursor =
                        self.current_tree_state().cursor.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if is_scroll_view {
                    self.current_scroll_state_mut().scroll_offset =
                        self.current_scroll_state().scroll_offset.saturating_add(1);
                } else {
                    let cursor = self.current_tree_state().cursor;
                    if cursor + 1 < row_count {
                        self.current_tree_state_mut().cursor = cursor + 1;
                    }
                }
            }
            KeyCode::Char('g') | KeyCode::Home => {
                if is_scroll_view {
                    self.current_scroll_state_mut().scroll_offset = 0;
                } else {
                    self.current_tree_state_mut().cursor = 0;
                }
            }
            KeyCode::Char('G') | KeyCode::End => {
                if is_scroll_view {
                    self.current_scroll_state_mut().scroll_offset = usize::MAX;
                } else {
                    self.current_tree_state_mut().cursor = row_count.saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                if let Some(row) = self.current_row() {
                    let id = row.nav.id;
                    let children_key = row.nav.children_key.clone();
                    let is_expanded = row.nav.is_expanded;
                    let has_children = row.nav.has_children;

                    if has_children {
                        if is_expanded {
                            self.collapse(id);
                        } else {
                            self.expand(id, children_key, snap);
                        }
                    }
                }
            }
            KeyCode::Right => {
                if let Some(row) = self.current_row() {
                    if row.nav.has_children && !row.nav.is_expanded {
                        let id = row.nav.id;
                        let children_key = row.nav.children_key.clone();
                        self.expand(id, children_key, snap);
                    }
                }
            }
            KeyCode::Left => {
                if let Some(row) = self.current_row() {
                    if row.nav.is_expanded {
                        self.collapse(row.nav.id);
                    } else {
                        // Go to parent
                        if let Some(parent_row) = row.nav.parent_row {
                            self.current_tree_state_mut().cursor = parent_row;
                        }
                    }
                }
            }
            KeyCode::Char('a') => {
                // Show all children in the current paged list.
                if let Some(parent) = self.find_paged_children_parent(snap) {
                    if let PagedChildrenParent::ClassMembers { agg_idx } = parent {
                        let state = self.current_tree_state_mut();
                        state.class_member_windows.insert(
                            agg_idx,
                            EdgeWindow {
                                start: 0,
                                count: usize::MAX,
                            },
                        );
                        self.recompute_paged_children(parent, snap);
                    } else {
                        let parent_id = match &parent {
                            PagedChildrenParent::Edges { id, .. }
                            | PagedChildrenParent::Retainers { id, .. } => *id,
                            PagedChildrenParent::ClassMembers { .. } => unreachable!(),
                        };
                        let state = self.current_tree_state_mut();
                        let mut w = state
                            .edge_windows
                            .get(&parent_id)
                            .copied()
                            .unwrap_or_default();
                        w.start = 0;
                        w.count = usize::MAX;
                        state.edge_windows.insert(parent_id, w);
                        self.recompute_paged_children(parent, snap);
                    }
                }
            }
            KeyCode::Char('v') => {
                // Switch from "selected" retainer paths to paginated view.
                if let Some(parent) = self.find_paged_children_parent(snap) {
                    if let PagedChildrenParent::Retainers { id, .. } = parent {
                        self.retainers.unfiltered_nodes.insert(id);
                        let state = self.current_tree_state_mut();
                        state.edge_windows.insert(id, EdgeWindow::default());
                        self.recompute_paged_children(parent, snap);
                    }
                }
            }
            KeyCode::Char('r') => {
                if let Some(row) = self.current_row() {
                    if let Some(ordinal) = row.node_ordinal() {
                        self.set_retainers_target(ordinal, snap);
                    }
                }
            }
            KeyCode::Char('s') => {
                if let Some(row) = self.current_row() {
                    if let Some(ordinal) = row.node_ordinal() {
                        self.show_in_summary(ordinal, snap);
                    }
                }
            }
            KeyCode::Char('d') => {
                if let Some(row) = self.current_row() {
                    if let Some(ordinal) = row.node_ordinal() {
                        self.show_in_dominators(ordinal, snap);
                    }
                }
            }
            KeyCode::Char('c') => {
                if let Some(row) = self.current_row() {
                    if let Some(ordinal) = row.node_ordinal() {
                        self.show_in_containment(ordinal, snap);
                    }
                }
            }
            KeyCode::Char('m') => {
                if let Some(row) = self.current_row() {
                    if let Some(ordinal) = row.node_ordinal() {
                        self.push_history(ordinal);
                    }
                }
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.adjust_edge_count(EDGE_PAGE_SIZE as isize, snap);
            }
            KeyCode::Char('-') => {
                self.adjust_edge_count(-(EDGE_PAGE_SIZE as isize), snap);
            }
            KeyCode::Char('u') => {
                if self.current_view == ViewType::Summary {
                    self.set_summary_filter(self.summary_filter_mode.next(), snap);
                }
            }
            KeyCode::Char('U') => {
                if self.current_view == ViewType::Summary {
                    self.set_summary_filter(self.summary_filter_mode.prev(), snap);
                }
            }
            KeyCode::Char('n') => {
                self.shift_edge_window(1, snap);
            }
            KeyCode::Char('p') => {
                self.shift_edge_window(-1, snap);
            }
            KeyCode::Char('f') => {
                if let Some(PagedChildrenParent::Edges {
                    id,
                    ordinal: ord,
                    is_compare,
                }) = self.find_paged_children_parent(snap)
                {
                    let existing = self
                        .current_tree_state()
                        .edge_filters
                        .get(&ord)
                        .cloned()
                        .unwrap_or_default();
                    self.edge_filter_target = Some(ord);
                    self.edge_filter_node_id = Some(id);
                    self.edge_filter_is_compare = is_compare;
                    self.edge_filter_input = existing;
                    self.input_mode = InputMode::EdgeFilter;
                }
            }
            KeyCode::Char('k') => {
                if self.current_view == ViewType::Retainers {
                    if let Some(row) = self.current_row() {
                        if let Some(ordinal) = row.node_ordinal() {
                            let id = row.nav.id;
                            let children_key = row.nav.children_key.clone();
                            if !row.nav.is_expanded && row.nav.has_children {
                                self.expand(id, children_key.clone(), snap);
                            }
                            if let Some(ck) = children_key {
                                self.queue_retainer_plan(ordinal, RetainerPlanKind::Subtree(ck));
                            }
                        }
                    }
                }
            }
            KeyCode::Char('R') => {
                if let Some(row) = self.current_row() {
                    if let Some(ordinal) = row.node_ordinal() {
                        self.queue_reachable(ordinal);
                    }
                }
            }
            KeyCode::Char('A') => {
                if let Some(row) = self.current_row() {
                    if let Some(ordinal) = row.node_ordinal() {
                        self.queue_reachable(ordinal);
                        for (_, child_ord) in snap.iter_edges(ordinal) {
                            self.queue_reachable(child_ord);
                        }
                    }
                }
            }
            KeyCode::Char('[') => {
                let state = self.current_tree_state_mut();
                state.horizontal_scroll = state
                    .horizontal_scroll
                    .saturating_sub(HORIZONTAL_SCROLL_STEP);
            }
            KeyCode::Char(']') => {
                let state = self.current_tree_state_mut();
                state.horizontal_scroll = state
                    .horizontal_scroll
                    .saturating_add(HORIZONTAL_SCROLL_STEP);
            }
            KeyCode::Char('{') => {
                if self.current_view == ViewType::Diff && self.diff.all_diffs.len() > 1 {
                    let idx = if self.diff.current_idx == 0 {
                        self.diff.all_diffs.len() - 1
                    } else {
                        self.diff.current_idx - 1
                    };
                    self.switch_diff(idx);
                }
            }
            KeyCode::Char('}') => {
                if self.current_view == ViewType::Diff && self.diff.all_diffs.len() > 1 {
                    let idx = (self.diff.current_idx + 1) % self.diff.all_diffs.len();
                    self.switch_diff(idx);
                }
            }
            _ => {}
        }
        false
    }

    fn handle_page_key(&mut self, key: KeyEvent, row_count: usize) -> bool {
        match key.code {
            KeyCode::PageUp => {
                self.page_up(row_count);
                true
            }
            KeyCode::PageDown => {
                self.page_down(row_count);
                true
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.page_up(row_count);
                true
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.page_down(row_count);
                true
            }
            _ => false,
        }
    }

    fn page_step(&self) -> usize {
        let height = if self.is_scroll_view() {
            self.current_scroll_state().page_height
        } else {
            self.current_tree_state().page_height
        };
        height.saturating_sub(1).max(1)
    }

    fn page_up(&mut self, row_count: usize) {
        if self.is_scroll_view() {
            let step = self.page_step();
            let state = self.current_scroll_state_mut();
            state.scroll_offset = state.scroll_offset.saturating_sub(step);
            return;
        }

        self.shift_page(row_count, false);
    }

    fn page_down(&mut self, row_count: usize) {
        if self.is_scroll_view() {
            let step = self.page_step();
            let state = self.current_scroll_state_mut();
            state.scroll_offset = state.scroll_offset.saturating_add(step);
            return;
        }

        self.shift_page(row_count, true);
    }

    fn shift_page(&mut self, row_count: usize, forward: bool) {
        if row_count == 0 {
            return;
        }

        let step = self.page_step();
        let (cursor, scroll_offset, page_height) = {
            let state = self.current_tree_state();
            (state.cursor, state.scroll_offset, state.page_height.max(1))
        };
        let relative_row = cursor
            .saturating_sub(scroll_offset)
            .min(page_height.saturating_sub(1));
        let max_scroll = row_count.saturating_sub(page_height);
        let new_scroll = if forward {
            (scroll_offset + step).min(max_scroll)
        } else {
            scroll_offset.saturating_sub(step)
        };

        let state = self.current_tree_state_mut();
        state.scroll_offset = new_scroll;
        state.cursor = (new_scroll + relative_row).min(row_count.saturating_sub(1));
    }
}
