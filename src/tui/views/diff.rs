use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::print::diff;
use crate::print::{format_count, format_size};
use crate::snapshot::HeapSnapshot;

use super::super::types::*;
use super::super::{
    App, COL_DIFF_NAME, COL_DIFF_NUM, COL_DIFF_SIZE, contains_ignore_case, fit_name_cell,
};

impl App {
    pub(in crate::tui) fn flatten_diff(
        &self,
        state: &TreeState,
        rows: &mut Vec<FlatRow>,
        snap: &HeapSnapshot,
    ) {
        let filter = &self.diff.filter;
        for (i, d) in self.diff.sorted_diffs.iter().enumerate() {
            if !filter.is_empty() && !contains_ignore_case(&d.name, filter) {
                continue;
            }
            let id = self.diff.diff_ids[i];
            let is_expanded = state.expanded.contains(&id);
            let has_children = !d.new_objects.is_empty() || !d.deleted_objects.is_empty();
            let count = d.new_count + d.deleted_count;
            let count_str = format!("\u{00d7}{}", format_count(count));
            let label = format!("{}  {count_str}", d.name);

            rows.push(FlatRow {
                nav: FlatRowNav {
                    id,
                    parent_row: None,
                    depth: 0,
                    has_children,
                    is_expanded,
                    children_key: if has_children {
                        Some(ChildrenKey::DiffMembers(i))
                    } else {
                        None
                    },
                },
                render: FlatRowRender {
                    label: label.into(),
                    is_weak: false,
                    is_root_holder: false,
                    inspect_source: None,
                    kind: FlatRowKind::DiffGroup {
                        new_count: d.new_count,
                        deleted_count: d.deleted_count,
                        alloc_size: d.alloc_size,
                        freed_size: d.freed_size,
                    },
                },
            });

            if is_expanded {
                let parent_row = rows.len() - 1;
                self.flatten_diff_children(
                    &ChildrenKey::DiffMembers(i),
                    parent_row,
                    state,
                    rows,
                    snap,
                );
            }
        }
    }

    pub(in crate::tui) fn compute_diff_members(
        &self,
        diff_idx: usize,
        snap: &HeapSnapshot,
    ) -> Vec<ChildNode> {
        let d = &self.diff.sorted_diffs[diff_idx];
        let mut children = Vec::new();

        // New objects — look up in main snapshot so they can be expanded
        for (node_id, self_size) in &d.new_objects {
            let ordinal = snap.node_for_snapshot_object_id(*node_id);
            let has_edges = ordinal.is_some_and(|o| snap.node_edge_count(o) > 0);
            let id = mint_id(&self.next_id);
            children.push(ChildNode {
                id,
                label: format!("+ {} @{node_id}", d.name).into(),
                distance: ordinal.map(|o| snap.node_distance(o)),
                shallow_size: *self_size as u64,
                retained_size: 0,
                node_ordinal: ordinal,
                has_children: has_edges,
                children_key: if has_edges {
                    ordinal.map(|o| ChildrenKey::Edges(id, o))
                } else {
                    None
                },
                is_weak: false,
                is_root_holder: false,
                inspect_source: None,
            });
        }

        // Deleted objects — look up in compare snapshot for expansion
        let compare_snap = self.diff.compare_snapshots.get(self.diff.current_idx);
        for (node_id, self_size) in &d.deleted_objects {
            let ordinal = compare_snap.and_then(|cs| cs.node_for_snapshot_object_id(*node_id));
            let has_edges =
                ordinal.is_some_and(|o| compare_snap.is_some_and(|cs| cs.node_edge_count(o) > 0));
            let id = mint_id(&self.next_id);
            children.push(ChildNode {
                id,
                label: format!("\u{2212} {} @{node_id}", d.name).into(),
                distance: ordinal.and_then(|o| compare_snap.map(|cs| cs.node_distance(o))),
                shallow_size: 0,
                retained_size: *self_size as u64,
                node_ordinal: ordinal,
                has_children: has_edges,
                children_key: if has_edges {
                    ordinal.map(|o| ChildrenKey::CompareEdges(id, o))
                } else {
                    None
                },
                is_weak: false,
                is_root_holder: false,
                inspect_source: None,
            });
        }

        children
    }

    pub(in crate::tui) fn flatten_diff_children(
        &self,
        children_key: &ChildrenKey,
        parent_row: usize,
        state: &TreeState,
        rows: &mut Vec<FlatRow>,
        snap: &HeapSnapshot,
    ) {
        let Some(children) = state.children_map.get(children_key) else {
            return;
        };
        for child in children {
            let is_expanded = state.expanded.contains(&child.id);
            let is_new = child.label.starts_with("+ ");

            rows.push(FlatRow {
                nav: FlatRowNav {
                    id: child.id,
                    parent_row: Some(parent_row),
                    depth: 1,
                    has_children: child.has_children,
                    is_expanded,
                    children_key: child.children_key.clone(),
                },
                render: FlatRowRender {
                    label: child.label.clone(),
                    is_weak: false,
                    is_root_holder: false,
                    inspect_source: None,
                    kind: FlatRowKind::DiffObject {
                        node_ordinal: child.node_ordinal,
                        is_new,
                        size: if is_new {
                            child.shallow_size
                        } else {
                            child.retained_size
                        },
                    },
                },
            });
            let child_row = rows.len() - 1;

            if is_expanded {
                if let Some(ref ck) = child.children_key {
                    self.flatten_children(ck, Some(child_row), 2, state, rows, snap);
                }
            }
        }
    }

    pub(in crate::tui) fn render_diff_column_header(&self, frame: &mut Frame, area: Rect) {
        let header = Line::from(vec![
            Span::styled(
                format!("{:<w$}", "Constructor", w = COL_DIFF_NAME),
                Style::default().bold(),
            ),
            Span::styled(
                format!("{:>w$}", "# New", w = COL_DIFF_NUM),
                Style::default().bold(),
            ),
            Span::styled(
                format!("{:>w$}", "# Deleted", w = COL_DIFF_NUM),
                Style::default().bold(),
            ),
            Span::styled(
                format!("{:>w$}", "# Delta", w = COL_DIFF_NUM),
                Style::default().bold(),
            ),
            Span::styled(
                format!("{:>w$}", "Alloc. Size", w = COL_DIFF_SIZE),
                Style::default().bold(),
            ),
            Span::styled(
                format!("{:>w$}", "Freed Size", w = COL_DIFF_SIZE),
                Style::default().bold(),
            ),
            Span::styled(
                format!("{:>w$}", "Size Delta", w = COL_DIFF_SIZE),
                Style::default().bold(),
            ),
        ]);
        frame.render_widget(Paragraph::new(header), Rect { height: 1, ..area });

        let sep_width =
            (COL_DIFF_NAME + COL_DIFF_NUM * 3 + COL_DIFF_SIZE * 3).min(area.width as usize);
        let sep = "\u{2500}".repeat(sep_width);
        frame.render_widget(
            Paragraph::new(sep),
            Rect {
                y: area.y + 1,
                height: 1,
                ..area
            },
        );
    }

    pub(in crate::tui) fn render_diff_rows(&mut self, frame: &mut Frame, area: Rect) {
        let tree_height = area.height as usize;
        let horizontal_scroll = self.clamp_horizontal_scroll(COL_DIFF_NAME);
        self.current_tree_state_mut().page_height = tree_height.max(1);

        // Adjust scroll to keep cursor visible
        let (cursor, old_scroll) = {
            let state = self.current_tree_state();
            (state.cursor, state.scroll_offset)
        };
        let scroll_offset = if cursor < old_scroll {
            cursor
        } else if cursor >= old_scroll + tree_height {
            cursor - tree_height + 1
        } else {
            old_scroll
        };
        self.current_tree_state_mut().scroll_offset = scroll_offset;

        let start = scroll_offset;
        let end = (start + tree_height).min(self.cached_rows.len());

        for (i, row) in self.cached_rows[start..end].iter().enumerate() {
            let y = area.y + i as u16;
            let is_selected = start + i == cursor;

            let bg = if is_selected {
                Color::DarkGray
            } else {
                Color::Reset
            };

            // Name column
            let indent = "  ".repeat(row.nav.depth);
            let marker = if row.nav.has_children {
                if row.nav.is_expanded {
                    "\u{25bc} "
                } else {
                    "\u{25b6} "
                }
            } else {
                "  "
            };
            let prefix = format!("{indent}{marker}");
            let name_col =
                fit_name_cell(&prefix, &row.render.label, COL_DIFF_NAME, horizontal_scroll);

            let line = match &row.render.kind {
                FlatRowKind::DiffGroup {
                    new_count,
                    deleted_count,
                    alloc_size,
                    freed_size,
                } => {
                    let delta_count = *new_count as i64 - *deleted_count as i64;
                    let size_delta = *alloc_size as i64 - *freed_size as i64;
                    Line::from(vec![
                        Span::styled(name_col, Style::default().fg(Color::White).bg(bg)),
                        Span::styled(
                            format!("{:>w$}", format_count(*new_count), w = COL_DIFF_NUM),
                            Style::default().bg(bg),
                        ),
                        Span::styled(
                            format!("{:>w$}", format_count(*deleted_count), w = COL_DIFF_NUM),
                            Style::default().bg(bg),
                        ),
                        Span::styled(
                            format!(
                                "{:>w$}",
                                diff::format_signed_count(delta_count),
                                w = COL_DIFF_NUM
                            ),
                            Style::default().bg(bg),
                        ),
                        Span::styled(
                            format!("{:>w$}", format_size(*alloc_size), w = COL_DIFF_SIZE),
                            Style::default().bg(bg),
                        ),
                        Span::styled(
                            format!("{:>w$}", format_size(*freed_size), w = COL_DIFF_SIZE),
                            Style::default().bg(bg),
                        ),
                        Span::styled(
                            format!(
                                "{:>w$}",
                                diff::format_signed_size(size_delta),
                                w = COL_DIFF_SIZE
                            ),
                            Style::default().bg(bg),
                        ),
                    ])
                }
                FlatRowKind::DiffObject { is_new, size, .. } => {
                    let (new_col, del_col, alloc_col, freed_col) = if *is_new {
                        (
                            "\u{2022}".to_string(),
                            String::new(),
                            format_size(*size),
                            String::new(),
                        )
                    } else {
                        (
                            String::new(),
                            "\u{2022}".to_string(),
                            String::new(),
                            format_size(*size),
                        )
                    };

                    Line::from(vec![
                        Span::styled(name_col, Style::default().fg(Color::White).bg(bg)),
                        Span::styled(
                            format!("{:>w$}", new_col, w = COL_DIFF_NUM),
                            Style::default().bg(bg),
                        ),
                        Span::styled(
                            format!("{:>w$}", del_col, w = COL_DIFF_NUM),
                            Style::default().bg(bg),
                        ),
                        Span::styled(
                            format!("{:>w$}", "", w = COL_DIFF_NUM),
                            Style::default().bg(bg),
                        ),
                        Span::styled(
                            format!("{:>w$}", alloc_col, w = COL_DIFF_SIZE),
                            Style::default().bg(bg),
                        ),
                        Span::styled(
                            format!("{:>w$}", freed_col, w = COL_DIFF_SIZE),
                            Style::default().bg(bg),
                        ),
                        Span::styled(
                            format!("{:>w$}", "", w = COL_DIFF_SIZE),
                            Style::default().bg(bg),
                        ),
                    ])
                }
                FlatRowKind::HeapNode { shallow_size, .. } => Line::from(vec![
                    Span::styled(name_col, Style::default().fg(Color::White).bg(bg)),
                    Span::styled(
                        format!("{:>w$}", format_size(*shallow_size), w = COL_DIFF_NUM),
                        Style::default().bg(bg),
                    ),
                    Span::styled(
                        format!("{:>w$}", "", w = COL_DIFF_NUM),
                        Style::default().bg(bg),
                    ),
                    Span::styled(
                        format!("{:>w$}", "", w = COL_DIFF_NUM),
                        Style::default().bg(bg),
                    ),
                    Span::styled(
                        format!("{:>w$}", "", w = COL_DIFF_SIZE),
                        Style::default().bg(bg),
                    ),
                    Span::styled(
                        format!("{:>w$}", "", w = COL_DIFF_SIZE),
                        Style::default().bg(bg),
                    ),
                    Span::styled(
                        format!("{:>w$}", "", w = COL_DIFF_SIZE),
                        Style::default().bg(bg),
                    ),
                ]),
                FlatRowKind::SummaryGroup { .. } => continue,
            };

            frame.render_widget(
                Paragraph::new(line),
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
            );
        }
    }
}
