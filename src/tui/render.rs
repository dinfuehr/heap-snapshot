use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::print::{format_size, pct_str};
use crate::snapshot::HeapSnapshot;

use super::types::*;
use super::{
    App, COL_DETACHED, COL_DIST, COL_REACHABLE, COL_REACHABLE_PCT, COL_RETAINED, COL_RETAINED_PCT,
    COL_SHALLOW, COL_SHALLOW_PCT, fit_cell, fit_name_cell, regular_name_col_width,
};

impl App {
    pub(super) fn render(&mut self, frame: &mut Frame, snap: &HeapSnapshot) {
        self.ensure_rows(snap);

        // Clamp cursor
        let row_count = self.cached_rows.len();
        let state = self.current_tree_state_mut();
        if row_count == 0 {
            state.cursor = 0;
        } else if state.cursor >= row_count {
            state.cursor = row_count - 1;
        }

        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(area);

        self.render_header(frame, chunks[0], snap);
        self.render_content(frame, chunks[1], snap);
        self.render_footer(frame, chunks[2]);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect, snap: &HeapSnapshot) {
        let mut tabs: Vec<(&str, ViewType)> = vec![
            ("1:Summary", ViewType::Summary),
            ("2:Containment", ViewType::Containment),
            ("3:Dominators", ViewType::Dominators),
            ("4:Retainers", ViewType::Retainers),
        ];
        if self.diff.has_diff {
            tabs.push(("5:Diff", ViewType::Diff));
        }
        tabs.push(("6:Contexts", ViewType::Contexts));
        tabs.push(("7:History", ViewType::History));
        tabs.push(("8:Statistics", ViewType::Statistics));
        tabs.push(("?:Help", ViewType::Help));

        let mut spans = Vec::new();
        for (label, view) in &tabs {
            let style = if *view == self.current_view {
                Style::default().bold().add_modifier(Modifier::REVERSED)
            } else {
                Style::default().dim()
            };
            spans.push(Span::styled(format!(" {label} "), style));
        }

        if self.current_view == ViewType::Retainers {
            if let Some(target) = self.retainers.target {
                spans.push(Span::styled("  ", Style::default()));
                spans.push(Span::styled(
                    format!(
                        "target: {} @{}",
                        snap.node_display_name(target),
                        snap.node_id(target)
                    ),
                    Style::default().dim(),
                ));
            }
        }

        if self.current_view == ViewType::Diff && self.diff.all_diffs.len() > 1 {
            spans.push(Span::styled("  ", Style::default()));
            spans.push(Span::styled(
                format!(
                    "comparing: {} ({}/{})",
                    self.diff.compare_names[self.diff.current_idx],
                    self.diff.current_idx + 1,
                    self.diff.all_diffs.len()
                ),
                Style::default().dim(),
            ));
            spans.push(Span::styled("  {}: prev/next", Style::default().dim()));
        }

        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn render_content(&mut self, frame: &mut Frame, area: Rect, snap: &HeapSnapshot) {
        if area.height < 3 {
            return;
        }

        if self.current_view == ViewType::Help {
            self.render_help(frame, area);
            return;
        }

        if self.current_view == ViewType::Statistics {
            self.render_statistics(frame, area, snap);
            return;
        }

        if self.current_view == ViewType::Diff {
            self.render_diff_column_header(frame, area);
        } else {
            self.render_column_header(frame, area);
        }

        let tree_area = Rect {
            y: area.y + 2,
            height: area.height.saturating_sub(2),
            ..area
        };
        if tree_area.height == 0 {
            return;
        }

        // Handle empty contexts view
        if self.current_view == ViewType::Contexts && self.contexts_ids.is_empty() {
            let msg = "No NativeContext objects found in this snapshot.";
            frame.render_widget(
                Paragraph::new(Span::styled(msg, Style::default().dim())),
                Rect {
                    height: 1,
                    ..tree_area
                },
            );
            return;
        }

        // Handle empty history view
        if self.current_view == ViewType::History && self.history.is_empty() {
            let msg = "No history yet. Navigate to objects via 'r' (retainers) or 's' (summary).";
            frame.render_widget(
                Paragraph::new(Span::styled(msg, Style::default().dim())),
                Rect {
                    height: 1,
                    ..tree_area
                },
            );
            return;
        }

        // Handle empty retainers view
        if self.current_view == ViewType::Retainers && self.retainers.target.is_none() {
            let msg =
                "No target selected. Press '/' and type @id to search, or press 'r' on any node.";
            frame.render_widget(
                Paragraph::new(Span::styled(msg, Style::default().dim())),
                Rect {
                    height: 1,
                    ..tree_area
                },
            );
            return;
        }

        if self.current_view == ViewType::Diff {
            self.render_diff_rows(frame, tree_area);
        } else {
            self.render_rows(frame, tree_area);
        }
    }

    fn render_column_header(&self, frame: &mut Frame, area: Rect) {
        let col_name = regular_name_col_width(area.width);
        let header = Line::from(vec![
            Span::styled(fit_cell("Object", col_name), Style::default().bold()),
            Span::styled(
                format!("{:>w$}", "Dist", w = COL_DIST),
                Style::default().bold(),
            ),
            Span::styled(
                format!("{:>w$}", "Shallow Size", w = COL_SHALLOW + COL_SHALLOW_PCT),
                Style::default().bold(),
            ),
            Span::styled(
                format!(
                    "{:>w$}",
                    "Retained Size",
                    w = COL_RETAINED + COL_RETAINED_PCT
                ),
                Style::default().bold(),
            ),
            Span::styled(
                format!(
                    "{:>w$}",
                    "Reachable Size",
                    w = COL_REACHABLE + COL_REACHABLE_PCT
                ),
                Style::default().bold(),
            ),
            Span::styled(
                format!("{:>w$}", "Det", w = COL_DETACHED),
                Style::default().bold(),
            ),
        ]);
        frame.render_widget(Paragraph::new(header), Rect { height: 1, ..area });

        let sep_width = area.width as usize;
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

    fn render_rows(&mut self, frame: &mut Frame, area: Rect) {
        let tree_height = area.height as usize;
        let col_name = regular_name_col_width(area.width);
        let horizontal_scroll = self.clamp_horizontal_scroll(col_name);
        self.current_tree_state_mut().page_height = tree_height.max(1);

        let (total_shallow, total_retained) = match self.current_view {
            ViewType::Summary => (self.summary_total_shallow, self.summary_total_retained),
            _ => (self.heap_total, self.heap_total),
        };

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
            let name_col = fit_name_cell(&prefix, &row.render.label, col_name, horizontal_scroll);

            let (distance, shallow_size, retained_size, reachable_size, detachedness, node_ordinal) =
                match &row.render.kind {
                    FlatRowKind::SummaryGroup {
                        distance,
                        shallow_size,
                        retained_size,
                    } => (*distance, *shallow_size, *retained_size, None, None, None),
                    FlatRowKind::HeapNode {
                        node_ordinal,
                        distance,
                        shallow_size,
                        retained_size,
                        reachable_size,
                        detachedness,
                    } => (
                        *distance,
                        *shallow_size,
                        *retained_size,
                        *reachable_size,
                        *detachedness,
                        *node_ordinal,
                    ),
                    FlatRowKind::DiffGroup { .. } | FlatRowKind::DiffObject { .. } => continue,
                };

            let dist_str = if distance < 0 {
                "\u{2013}".to_string()
            } else {
                distance.to_string()
            };
            let shallow_str = format_size(shallow_size);
            let shallow_pct = pct_str(shallow_size, total_shallow);
            let retained_str = format_size(retained_size);
            let retained_pct = pct_str(retained_size, total_retained);
            let is_reachable_pending =
                node_ordinal.is_some_and(|o| self.reachable_pending.contains(&o));
            let (reachable_str, reachable_pct) = if is_reachable_pending {
                ("\u{2026}".to_string(), String::new())
            } else {
                match reachable_size {
                    Some(sz) => (format_size(sz), pct_str(sz, total_retained)),
                    None => ("\u{2013}".to_string(), String::new()),
                }
            };

            let is_status_line = distance < 0 && !row.nav.has_children;
            let name_style = if row.render.is_root_holder {
                Style::default().fg(Color::Red).bg(bg)
            } else if row.render.is_weak {
                Style::default().fg(Color::Cyan).bg(bg)
            } else if is_status_line {
                Style::default().fg(Color::Yellow).bg(bg)
            } else {
                Style::default().fg(Color::White).bg(bg)
            };

            let line = Line::from(vec![
                Span::styled(name_col, name_style),
                Span::styled(
                    format!("{:>w$}", dist_str, w = COL_DIST),
                    Style::default().bg(bg),
                ),
                Span::styled(
                    format!("{:>w$}", shallow_str, w = COL_SHALLOW),
                    Style::default().bg(bg),
                ),
                Span::styled(
                    format!("{:>w$}", shallow_pct, w = COL_SHALLOW_PCT),
                    Style::default().bg(bg),
                ),
                Span::styled(
                    format!("{:>w$}", retained_str, w = COL_RETAINED),
                    Style::default().bg(bg),
                ),
                Span::styled(
                    format!("{:>w$}", retained_pct, w = COL_RETAINED_PCT),
                    Style::default().bg(bg),
                ),
                Span::styled(
                    format!("{:>w$}", reachable_str, w = COL_REACHABLE),
                    Style::default().bg(bg),
                ),
                Span::styled(
                    format!("{:>w$}", reachable_pct, w = COL_REACHABLE_PCT),
                    Style::default().bg(bg),
                ),
                Span::styled(
                    format!(
                        "{:>w$}",
                        match detachedness {
                            Some(1) => "no",
                            Some(2) => "yes",
                            Some(_) => "?",
                            None => "\u{2013}",
                        },
                        w = COL_DETACHED
                    ),
                    Style::default().bg(bg),
                ),
            ]);

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

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let line = match self.input_mode {
            InputMode::Search => {
                let prompt = format!("/{}\u{2588}", self.search_input);
                Line::from(Span::styled(prompt, Style::default().fg(Color::Yellow)))
            }
            InputMode::EdgeFilter => {
                let prompt = format!("filter edges: {}\u{2588}", self.edge_filter_input);
                Line::from(Span::styled(prompt, Style::default().fg(Color::Yellow)))
            }
            InputMode::Normal => {
                if self.current_view == ViewType::Help {
                    Line::from(Span::styled(
                        "q:quit  ?:help  Tab:cycle views  \u{2191}\u{2193}/PgUp/PgDn/C-b/C-f/Home/End:scroll",
                        Style::default().dim(),
                    ))
                } else if let Some(ref err) = self.search_error {
                    Line::from(Span::styled(err.clone(), Style::default().fg(Color::Red)))
                } else if self.current_view == ViewType::Summary
                    && (self.summary_unreachable_only || !self.summary_filter.is_empty())
                {
                    let mut spans = Vec::new();
                    if self.summary_unreachable_only {
                        spans.push(Span::styled(
                            "unreachable only",
                            Style::default().fg(Color::Yellow),
                        ));
                    }
                    if !self.summary_filter.is_empty() {
                        if !spans.is_empty() {
                            spans.push(Span::raw(" + "));
                        }
                        spans.push(Span::styled(
                            format!("filter: \"{}\"", self.summary_filter),
                            Style::default().fg(Color::Yellow),
                        ));
                    }
                    spans.push(Span::styled(
                        "  (u: toggle unreachable, /: text filter)",
                        Style::default().dim(),
                    ));
                    Line::from(spans)
                } else if !self.diff.filter.is_empty() && self.current_view == ViewType::Diff {
                    Line::from(vec![
                        Span::styled(
                            format!("filter: \"{}\"", self.diff.filter),
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::styled("  (/ to change, empty to clear)", Style::default().dim()),
                    ])
                } else if self.current_view == ViewType::Retainers
                    && self.retainers.plan_pending.is_some()
                {
                    Line::from(Span::styled(
                        "Computing retainer paths to (GC roots)...",
                        Style::default().dim(),
                    ))
                } else if self.current_view == ViewType::Retainers {
                    if let Some(ref msg) = self.retainers.plan_message {
                        Line::from(Span::styled(msg.clone(), Style::default().dim()))
                    } else {
                        let views = if self.diff.has_diff {
                            "1/2/3/4/5/6:views"
                        } else {
                            "1/2/3/4/6:views"
                        };
                        let extra_hints = match self.current_view {
                            ViewType::Diff if self.diff.all_diffs.len() > 1 => "  {}:snapshot",
                            ViewType::Retainers => "  k:auto-expand",
                            _ => "",
                        };
                        Line::from(Span::styled(
                            format!(
                                "q:quit  {views}  Tab:cycle  /:search  [ ]:pan  \u{2191}\u{2193}:navigate  \u{2190}\u{2192}:collapse/expand  Enter:toggle  a:all-refs  r:retainers  s:summary  R/A:reachable{extra_hints}"
                            ),
                            Style::default().dim(),
                        ))
                    }
                } else {
                    let views = if self.diff.has_diff {
                        "1/2/3/4/5/6:views"
                    } else {
                        "1/2/3/4/6:views"
                    };
                    let extra_hints = match self.current_view {
                        ViewType::Diff if self.diff.all_diffs.len() > 1 => "  {}:snapshot",
                        ViewType::Retainers => "  k:auto-expand",
                        _ => "",
                    };
                    Line::from(Span::styled(
                        format!(
                            "q:quit  {views}  Tab:cycle  /:search  [ ]:pan  \u{2191}\u{2193}:navigate  \u{2190}\u{2192}:collapse/expand  Enter:toggle  a:all-refs  r:retainers  s:summary  R/A:reachable{extra_hints}"
                        ),
                        Style::default().dim(),
                    ))
                }
            }
        };
        frame.render_widget(Paragraph::new(line), area);
    }
}
