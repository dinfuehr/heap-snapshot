use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::snapshot::HeapSnapshot;
use crate::tui::App;

impl App {
    pub(in crate::tui) fn render_action_menu_overlay(
        &self,
        frame: &mut Frame,
        area: Rect,
        snap: &HeapSnapshot,
    ) {
        let Some(target) = self.action_menu.target else {
            return;
        };
        let target_id = snap.node_id(target);

        let labels: Vec<String> = self
            .action_menu
            .actions
            .iter()
            .map(|a| a.label(target_id))
            .collect();

        let max_label = labels.iter().map(|l| l.len()).max().unwrap_or(0);
        // +4 for border + leading space and marker; cap to visible area.
        let width = (max_label as u16 + 6).min(area.width);
        let height = (labels.len() as u16 + 2).min(area.height); // +2 for border

        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;
        let overlay_area = Rect {
            x,
            y,
            width,
            height,
        };

        frame.render_widget(Clear, overlay_area);

        let block = Block::default().borders(Borders::ALL).title(" Actions ");
        let inner = block.inner(overlay_area);
        frame.render_widget(block, overlay_area);

        for (i, label) in labels.iter().enumerate() {
            if i as u16 >= inner.height {
                break;
            }
            let row_area = Rect {
                x: inner.x,
                y: inner.y + i as u16,
                width: inner.width,
                height: 1,
            };
            let marker = if i == self.action_menu.cursor {
                "> "
            } else {
                "  "
            };
            let style = if i == self.action_menu.cursor {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            frame.render_widget(
                Paragraph::new(Span::styled(format!("{marker}{label}"), style)),
                row_area,
            );
        }
    }
}
