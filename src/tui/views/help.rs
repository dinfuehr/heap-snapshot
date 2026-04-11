use ratatui::prelude::*;
use ratatui::widgets::{Paragraph, Wrap};

use super::super::App;

impl App {
    pub(in crate::tui) fn render_help(&mut self, frame: &mut Frame, area: Rect) {
        let lines = self.help_lines();
        let max_scroll = lines.len().saturating_sub(area.height as usize);
        let state = &mut self.help_state;
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

    fn help_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![
            Line::from(Span::styled(
                "Heap Snapshot TUI Help",
                Style::default().bold(),
            )),
            Line::from(""),
            Line::from(Span::styled("Views", Style::default().bold())),
            Line::from("  1 Summary, 2 Containment, 3 Dominators, 4 Retainers"),
            Line::from(if self.diff.has_diff {
                "  5 Diff, 6 Contexts, 7 History, 8 Statistics, 9 Timeline, ? Help"
            } else {
                "  6 Contexts, 7 History, 8 Statistics, 9 Timeline, ? Help"
            }),
            Line::from("  Tab / Shift-Tab cycle between views"),
            Line::from(""),
            Line::from(Span::styled("Navigation", Style::default().bold())),
            Line::from("  Up/Down move, PageUp/PageDown or Ctrl-B/Ctrl-F jump"),
            Line::from("  Home/End go to top/bottom"),
            Line::from("  Left/Right or Enter collapse/expand the selected row"),
            Line::from("  [ / ] pan the name column horizontally for long labels"),
            Line::from(""),
            Line::from(Span::styled("Search And Filters", Style::default().bold())),
            Line::from("  / opens search prompt (Summary and Diff views)"),
            Line::from("    text filters by constructor name, @id jumps to an object"),
            Line::from("  f filters the current edge list; empty input clears it"),
            Line::from("  u / U cycles summary filter forward / backward"),
            Line::from(
                "    (All, Unreachable, Unreachable roots, Detached DOM, Console, Event handlers)",
            ),
            Line::from("  Enter applies a prompt, Esc cancels, Backspace deletes"),
            Line::from(""),
            Line::from(Span::styled("Inspection", Style::default().bold())),
            Line::from("  r opens Retainers for the selected object"),
            Line::from("  s jumps the selected object back to Summary"),
            Line::from("  d opens the selected object in Dominators view"),
            Line::from("  c opens the selected object in Containment view"),
            Line::from("  m remembers the selected object (adds to History)"),
            Line::from("  R computes Reachable Size for the selected object"),
            Line::from("  A computes Reachable Size for the selected object and its outgoing refs"),
            Line::from(""),
            Line::from(Span::styled("Edges", Style::default().bold())),
            Line::from("  a shows all refs for the current edge list"),
            Line::from("  + / - grow or shrink the current page size"),
            Line::from("  n / p move to the next or previous page"),
            Line::from(""),
            Line::from(Span::styled("Retainers", Style::default().bold())),
            Line::from("  On load, the retainer tree auto-expands a path toward (GC roots)"),
            Line::from("  when one is found within the search limits (depth 20, 2000 nodes)."),
            Line::from("  k auto-expands paths from the current node toward (GC roots)"),
            Line::from(vec![
                Span::raw("  "),
                Span::styled("Red", Style::default().fg(Color::Red)),
                Span::raw(" names mark GC root holders; "),
                Span::styled("cyan", Style::default().fg(Color::Cyan)),
                Span::raw(" marks weak edges (even on GC roots)."),
            ]),
        ];

        if self.diff.has_diff {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("Diff", Style::default().bold())));
            lines.push(Line::from("  { / } switch the compare snapshot"));
        }

        lines.extend([
            Line::from(""),
            Line::from(Span::styled("Metrics", Style::default().bold())),
            Line::from("  Retained Size: memory that would become collectable if this object"),
            Line::from("  and everything it uniquely keeps alive were removed from the graph."),
            Line::from("  Reachable Size: sum of shallow sizes reachable by following outgoing"),
            Line::from("  references from this object, ignoring weak and shortcut edges."),
            Line::from("  Distance: BFS hops from GC roots. Unreachable objects show U, U+1, etc."),
            Line::from("    U: directly unreachable — referenced only via weak/filtered edges."),
            Line::from("    U+N: N hops from a U object through the unreachable subgraph."),
            Line::from("  Det: 0 unknown, 1 attached, 2 detached."),
            Line::from(""),
            Line::from(Span::styled(
                "Contexts And Window Types",
                Style::default().bold(),
            )),
            Line::from("  V8 snapshots carry a NativeContext tag from the embedder, typically"),
            Line::from("  a URL or thread/context name. V8 does not emit an explicit frame"),
            Line::from("  type for a window."),
            Line::from("  (global*): JSGlobalObject. (global): JSGlobalProxy."),
            Line::from("  In Blink this is usually Window (global*) / Window (global)."),
            Line::from("  This tool infers the prefix as follows:"),
            Line::from("  [main]: Window global_object, and its global proxy has many refs."),
            Line::from("  [iframe]: Window global_object, but the proxy looks small."),
            Line::from("  [utility]: no Window global_object was found for the context."),
        ]);

        lines
    }
}
