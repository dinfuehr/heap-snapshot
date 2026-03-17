use ratatui::prelude::*;
use ratatui::widgets::{Paragraph, Wrap};

use crate::print::format_size;
use crate::snapshot::HeapSnapshot;

use super::super::App;

impl App {
    pub(in crate::tui) fn render_statistics(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        snap: &HeapSnapshot,
    ) {
        let stats = snap.get_statistics();
        let total = stats.total;

        // Categories with colors — ordered by typical dominance
        let v8_other =
            (stats.v8heap_total - stats.code - stats.strings - stats.js_arrays - stats.system)
                .max(0.0);
        let categories: Vec<(&str, f64, Color)> = vec![
            ("V8 Heap", v8_other, Color::Blue),
            ("Code", stats.code, Color::Yellow),
            ("Strings", stats.strings, Color::Green),
            ("JS Arrays", stats.js_arrays, Color::Cyan),
            ("System", stats.system, Color::Magenta),
            ("Native", stats.native_total, Color::Red),
        ];

        let mut lines: Vec<Line<'static>> = Vec::new();

        lines.push(Line::from(Span::styled(
            "Heap Statistics",
            Style::default().bold(),
        )));
        lines.push(Line::from(""));

        // Total
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<16}", "Total"), Style::default().bold()),
            Span::raw(format_size(total)),
        ]));
        lines.push(Line::from(""));

        // Individual categories
        for (name, value, color) in &categories {
            let pct = if total > 0.0 {
                format!("{:>5.1}%", value / total * 100.0)
            } else {
                " 0.0%".to_string()
            };
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(format!("{:<16}", name), Style::default().fg(*color).bold()),
                Span::raw(format!("{:>12}  {}", format_size(*value), pct)),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(""));

        // Typed arrays (sub-category of native)
        if stats.typed_arrays > 0.0 {
            let pct = if total > 0.0 {
                format!("{:>5.1}%", stats.typed_arrays / total * 100.0)
            } else {
                " 0.0%".to_string()
            };
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(format!("{:<16}", "Typed Arrays"), Style::default().dim()),
                Span::raw(format!(
                    "{:>12}  {}  (subset of Native)",
                    format_size(stats.typed_arrays),
                    pct
                )),
            ]));
            lines.push(Line::from(""));
        }

        // Unreachable objects
        if stats.unreachable_count > 0 {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!("{:<16}", "Unreachable"),
                    Style::default().fg(Color::DarkGray).bold(),
                ),
                Span::raw(format!(
                    "{:>12}  ({} objects)",
                    format_size(stats.unreachable_size),
                    crate::print::format_count(stats.unreachable_count),
                )),
            ]));
            lines.push(Line::from(""));
        }

        // Colored proportional bar
        let bar_width = (area.width as usize).saturating_sub(4);
        if bar_width > 0 && total > 0.0 {
            let mut bar_spans: Vec<Span<'static>> = vec![Span::raw("  ")];
            let mut used = 0usize;
            for (i, (_name, value, color)) in categories.iter().enumerate() {
                let frac = value / total;
                let w = if i == categories.len() - 1 {
                    // Last segment takes remaining width to avoid rounding gaps
                    bar_width.saturating_sub(used)
                } else {
                    (frac * bar_width as f64).round() as usize
                };
                if w > 0 {
                    bar_spans.push(Span::styled(
                        "\u{2588}".repeat(w),
                        Style::default().fg(*color),
                    ));
                    used += w;
                }
            }
            lines.push(Line::from(bar_spans));

            // Legend
            lines.push(Line::from(""));
            let mut legend_spans: Vec<Span<'static>> = vec![Span::raw("  ")];
            for (i, (name, _value, color)) in categories.iter().enumerate() {
                if i > 0 {
                    legend_spans.push(Span::raw("  "));
                }
                legend_spans.push(Span::styled("\u{2588}", Style::default().fg(*color)));
                legend_spans.push(Span::raw(format!(" {name}")));
            }
            lines.push(Line::from(legend_spans));
        }

        let max_scroll = lines.len().saturating_sub(area.height as usize);
        let state = &mut self.statistics_state;
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
