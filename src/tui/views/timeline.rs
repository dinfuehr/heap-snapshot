use ratatui::prelude::*;
use ratatui::widgets::{Paragraph, Wrap};

use crate::print::format_size;
use crate::snapshot::HeapSnapshot;

use super::super::App;

impl App {
    pub(in crate::tui) fn render_timeline(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        snap: &HeapSnapshot,
    ) {
        let intervals = snap.get_timeline();

        if intervals.is_empty() {
            frame.render_widget(
                Paragraph::new("No allocation timeline data in this snapshot."),
                area,
            );
            return;
        }

        let max_size = intervals.iter().map(|i| i.size).max().unwrap_or(1).max(1);
        let total_count: u64 = intervals.iter().map(|i| i.count as u64).sum();
        let total_size: u64 = intervals.iter().map(|i| i.size).sum();
        let bar_width = (area.width as usize).saturating_sub(30);

        let mut lines: Vec<Line<'static>> = Vec::new();

        lines.push(Line::from(Span::styled(
            "Allocation Timeline",
            Style::default().bold(),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(format!(
            "  {} intervals, {} live objects, {} total",
            intervals.len(),
            total_count,
            format_size(total_size),
        )));
        lines.push(Line::from(""));

        // Draw bars
        for interval in intervals {
            let w = if max_size > 0 {
                ((interval.size as f64 / max_size as f64) * bar_width as f64).round() as usize
            } else {
                0
            };

            let ts_sec = interval.timestamp_us as f64 / 1_000_000.0;
            let label = format!("{:>6.1}s {:>5} ", ts_sec, format_size(interval.size));
            let bar = "\u{2588}".repeat(w);

            let color = if interval.count == 0 {
                Color::DarkGray
            } else {
                Color::Blue
            };

            lines.push(Line::from(vec![
                Span::raw(label),
                Span::styled(bar, Style::default().fg(color)),
            ]));
        }

        let max_scroll = lines.len().saturating_sub(area.height as usize);
        let state = &mut self.timeline_state;
        state.page_height = area.height.max(1) as usize;
        if state.scroll_offset > max_scroll {
            state.scroll_offset = max_scroll;
        }

        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((state.scroll_offset as u16, 0)),
            area,
        );
    }
}
