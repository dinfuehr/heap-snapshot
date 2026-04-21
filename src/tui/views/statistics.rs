use ratatui::prelude::*;
use ratatui::widgets::{Paragraph, Wrap};

use crate::display::truncate_str;
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
        let v8_other = stats
            .v8heap_total
            .saturating_sub(stats.code + stats.strings + stats.js_arrays + stats.system);
        let categories: Vec<(&str, u64, Color)> = vec![
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
            let pct = if total > 0 {
                format!("{:>5.1}%", *value as f64 / total as f64 * 100.0)
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

        // Extra native bytes (sub-category of native)
        {
            let pct = if total > 0 {
                format!(
                    "{:>5.1}%",
                    stats.extra_native_bytes as f64 / total as f64 * 100.0
                )
            } else {
                " 0.0%".to_string()
            };
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(format!("{:<16}", "Extra Native"), Style::default().dim()),
                Span::raw(format!(
                    "{:>12}  {}  (subset of Native)",
                    format_size(stats.extra_native_bytes),
                    pct
                )),
            ]));
            lines.push(Line::from(""));
        }

        // Typed arrays (sub-category of native)
        if stats.typed_arrays > 0 {
            let pct = if total > 0 {
                format!("{:>5.1}%", stats.typed_arrays as f64 / total as f64 * 100.0)
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
        {
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
        if bar_width > 0 && total > 0 {
            let mut bar_spans: Vec<Span<'static>> = vec![Span::raw("  ")];
            let mut used = 0usize;
            for (i, (_name, value, color)) in categories.iter().enumerate() {
                let frac = *value as f64 / total as f64;
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

        // Native Context Attribution
        let contexts = snap.native_contexts();
        if !contexts.is_empty() {
            let ctx_colors = [
                Color::Blue,
                Color::Red,
                Color::Green,
                Color::Yellow,
                Color::Magenta,
                Color::LightRed,
                Color::Cyan,
                Color::LightYellow,
                Color::LightMagenta,
                Color::LightGreen,
            ];
            let shared = snap.shared_attributable_size();
            let unattributed = snap.unattributed_size();
            let attr_total: u64 =
                contexts.iter().map(|c| c.size).sum::<u64>() + shared + unattributed;

            lines.push(Line::from(""));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Native Context Attribution",
                Style::default().bold(),
            )));
            lines.push(Line::from(""));

            // indent(2) + label + size(12) + gap(2) + pct(6) + gap(2) = 24 fixed
            let max_label_width = (area.width as usize).saturating_sub(24).max(16);
            let content_width = contexts
                .iter()
                .map(|ctx| snap.native_context_label(ctx.ordinal).chars().count())
                .chain(std::iter::once("Unattributed".len()))
                .max()
                .unwrap_or(0);
            let label_width = content_width.min(max_label_width);

            // Per-context rows
            for (i, ctx) in contexts.iter().enumerate() {
                let label = truncate_str(&snap.native_context_label(ctx.ordinal), label_width);
                let color = ctx_colors[i % ctx_colors.len()];
                let pct = if attr_total > 0 {
                    format!("{:>5.1}%", ctx.size as f64 / attr_total as f64 * 100.0)
                } else {
                    " 0.0%".to_string()
                };
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(
                        format!("{:<w$}", label, w = label_width),
                        Style::default().fg(color).bold(),
                    ),
                    Span::raw(format!("{:>12}  {}", format_size(ctx.size), pct)),
                ]));
            }

            // Shared
            {
                let pct = if attr_total > 0 {
                    format!("{:>5.1}%", shared as f64 / attr_total as f64 * 100.0)
                } else {
                    " 0.0%".to_string()
                };
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(
                        format!("{:<w$}", "Shared", w = label_width),
                        Style::default().fg(Color::DarkGray).bold(),
                    ),
                    Span::raw(format!("{:>12}  {}", format_size(shared), pct)),
                ]));
            }

            // Unattributed
            {
                let pct = if attr_total > 0 {
                    format!("{:>5.1}%", unattributed as f64 / attr_total as f64 * 100.0)
                } else {
                    " 0.0%".to_string()
                };
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(
                        format!("{:<w$}", "Unattributed", w = label_width),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(format!("{:>12}  {}", format_size(unattributed), pct)),
                ]));
            }

            // Colored bar
            lines.push(Line::from(""));
            if bar_width > 0 && attr_total > 0 {
                let mut bar_spans: Vec<Span<'static>> = vec![Span::raw("  ")];
                let mut used = 0usize;
                let all_segments: Vec<(u64, Color)> = contexts
                    .iter()
                    .enumerate()
                    .map(|(i, ctx)| (ctx.size, ctx_colors[i % ctx_colors.len()]))
                    .chain(std::iter::once((shared, Color::DarkGray)))
                    .chain(std::iter::once((unattributed, Color::Gray)))
                    .collect();
                for (i, (value, color)) in all_segments.iter().enumerate() {
                    let frac = *value as f64 / attr_total as f64;
                    let w = if i == all_segments.len() - 1 {
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
                for (i, ctx) in contexts.iter().enumerate() {
                    if i > 0 {
                        legend_spans.push(Span::raw("  "));
                    }
                    let color = ctx_colors[i % ctx_colors.len()];
                    let short = truncate_str(&snap.native_context_label(ctx.ordinal), 20);
                    legend_spans.push(Span::styled("\u{2588}", Style::default().fg(color)));
                    legend_spans.push(Span::raw(format!(" {short}")));
                }
                legend_spans.push(Span::raw("  "));
                legend_spans.push(Span::styled(
                    "\u{2588}",
                    Style::default().fg(Color::DarkGray),
                ));
                legend_spans.push(Span::raw(" Shared"));
                legend_spans.push(Span::raw("  "));
                legend_spans.push(Span::styled("\u{2588}", Style::default().fg(Color::Gray)));
                legend_spans.push(Span::raw(" Unattributed"));
                lines.push(Line::from(legend_spans));
            }
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
