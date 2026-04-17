use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::tui::App;

impl App {
    pub(in crate::tui) fn render_inspect_overlay(&self, frame: &mut Frame, area: Rect) {
        let line_count = self.inspect_lines.len();
        let max_width = self
            .inspect_lines
            .iter()
            .map(|l| l.len())
            .max()
            .unwrap_or(0);

        let height = (line_count as u16 + 2).min(area.height); // +2 for border
        let width = (max_width as u16 + 4).min(area.width); // +4 for border + padding

        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;

        let overlay_area = Rect {
            x,
            y,
            width,
            height,
        };

        frame.render_widget(Clear, overlay_area);

        let block = Block::default().borders(Borders::ALL).title(" Inspect ");
        let inner = block.inner(overlay_area);
        frame.render_widget(block, overlay_area);

        for (i, line) in self.inspect_lines.iter().enumerate() {
            if i as u16 >= inner.height {
                break;
            }
            let row_area = Rect {
                x: inner.x,
                y: inner.y + i as u16,
                width: inner.width,
                height: 1,
            };
            let style = if line == "Node" || line == "Edge (from parent)" {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            frame.render_widget(Paragraph::new(Span::styled(line.as_str(), style)), row_area);
        }
    }
}
