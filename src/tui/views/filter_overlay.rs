use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::tui::types::FilterOverlayItem;
use crate::tui::{truncate_str, App};

impl App {
    pub(in crate::tui) fn render_filter_overlay(&mut self, frame: &mut Frame, area: Rect) {
        let item_count = self.filter_overlay_items.len();
        let max_visible = (area.height as usize).saturating_sub(2); // border top + bottom
        let visible = item_count.min(max_visible);
        let total_height = (visible + 2) as u16;

        let margin_x = 2u16.min(area.width / 4);
        let overlay_width = area.width.saturating_sub(margin_x * 2);
        let y_offset = (area.height.saturating_sub(total_height)) / 2;

        let overlay_area = Rect {
            x: area.x + margin_x,
            y: area.y + y_offset,
            width: overlay_width,
            height: total_height,
        };

        frame.render_widget(Clear, overlay_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Select Filter ");
        let inner = block.inner(overlay_area);
        frame.render_widget(block, overlay_area);

        // Scroll adjustment: keep at least 3 rows of context above/below cursor
        let margin = 3usize.min(visible / 2);
        if self.filter_overlay_cursor < self.filter_overlay_scroll + margin {
            self.filter_overlay_scroll = self.filter_overlay_cursor.saturating_sub(margin);
        }
        if self.filter_overlay_cursor + margin >= self.filter_overlay_scroll + visible {
            self.filter_overlay_scroll = (self.filter_overlay_cursor + margin + 1)
                .saturating_sub(visible)
                .min(item_count.saturating_sub(visible));
        }

        let start = self.filter_overlay_scroll;
        let end = (start + visible).min(item_count);

        let has_more_above = start > 0;
        let has_more_below = end < item_count;

        for (i, item) in self.filter_overlay_items[start..end].iter().enumerate() {
            let row_width = if has_more_above || has_more_below {
                (inner.width as usize).saturating_sub(2)
            } else {
                inner.width as usize
            };
            let row_area = Rect {
                x: inner.x,
                y: inner.y + i as u16,
                width: inner.width,
                height: 1,
            };
            let idx = start + i;

            match item {
                FilterOverlayItem::Header(title) => {
                    let text = truncate_str(title, row_width);
                    frame.render_widget(
                        Paragraph::new(Span::styled(text, Style::default().dim())),
                        row_area,
                    );
                }
                FilterOverlayItem::Filter { label, mode } => {
                    let is_cursor = idx == self.filter_overlay_cursor;
                    let is_active = *mode == self.summary_filter_mode;
                    let marker = if is_active { "\u{25cf} " } else { "  " };
                    let full = format!("{marker}{label}");
                    let text = truncate_str(&full, row_width);

                    let style = if is_cursor {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };

                    frame.render_widget(Paragraph::new(Span::styled(text, style)), row_area);
                }
            }

            // Scroll indicators on the right edge
            if (has_more_above || has_more_below) && inner.width >= 2 {
                let indicator = if i == 0 && has_more_above {
                    "\u{25b2}" // ▲
                } else if i + 1 == visible && has_more_below {
                    "\u{25bc}" // ▼
                } else {
                    ""
                };
                if !indicator.is_empty() {
                    frame.render_widget(
                        Paragraph::new(Span::styled(indicator, Style::default().dim())),
                        Rect {
                            x: inner.x + inner.width - 1,
                            y: inner.y + i as u16,
                            width: 1,
                            height: 1,
                        },
                    );
                }
            }
        }
    }
}
