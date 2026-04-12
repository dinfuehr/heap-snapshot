use rustc_hash::{FxHashMap, FxHashSet};
use std::cell::Cell;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::prelude::*;

use crate::print::diff;
use crate::print::retainers::{RetainerAutoExpandLimits, plan_gc_root_retainer_paths};
use crate::print::{display_width, pad_str, slice_str, truncate_str};
use crate::snapshot::HeapSnapshot;
use crate::types::{AggregateInfo, NodeOrdinal};

mod types;
use types::*;

mod children;
use children::compute_children;

mod app;
mod flatten;
mod history;
mod input;
mod render;
mod views;
use history::{load_extension_names, load_history, save_extension_names, save_history};

pub mod bench;

#[cfg(test)]
mod tests;

// ── Column widths ────────────────────────────────────────────────────────

const COL_DIST: usize = 8;
const COL_SHALLOW: usize = 12;
const COL_SHALLOW_PCT: usize = 5;
const COL_RETAINED: usize = 12;
const COL_RETAINED_PCT: usize = 5;
const COL_REACHABLE: usize = 14;
const COL_REACHABLE_PCT: usize = 5;
const COL_DETACHED: usize = 5;

// Diff view column widths (match print/diff.rs)
const COL_DIFF_NAME: usize = 40;
const COL_DIFF_NUM: usize = 12;
const COL_DIFF_SIZE: usize = 14;
const RETAINER_AUTO_EXPAND_DEPTH: usize = 20;
const RETAINER_AUTO_EXPAND_NODES: usize = 2000;
const HORIZONTAL_SCROLL_STEP: usize = 8;
const EDGE_PAGE_SIZE: usize = 20;

fn regular_fixed_col_width() -> usize {
    COL_DIST
        + COL_SHALLOW
        + COL_SHALLOW_PCT
        + COL_RETAINED
        + COL_RETAINED_PCT
        + COL_REACHABLE
        + COL_REACHABLE_PCT
        + COL_DETACHED
}

fn regular_name_col_width(area_width: u16) -> usize {
    (area_width as usize).saturating_sub(regular_fixed_col_width())
}

fn fit_cell(text: &str, width: usize) -> String {
    pad_str(&truncate_str(text, width), width)
}

fn fit_name_cell(prefix: &str, label: &str, width: usize, scroll: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let full = format!("{prefix}{label}");
    fit_cell(&slice_str(&full, scroll, width), width)
}

/// Case-insensitive substring check without allocating a lowercase copy.
/// `needle` must already be lowercase.
fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let needle_len = needle.len();
    if haystack.len() < needle_len {
        return false;
    }
    haystack
        .as_bytes()
        .windows(needle_len)
        .any(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
}

use crate::snapshot::NativeContextId;

/// Summary view filter mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SummaryFilterMode {
    /// Show all objects (no filter).
    All,
    /// Show all unreachable objects (distance >= UNREACHABLE_BASE).
    Unreachable,
    /// Show only fully unreachable roots (distance == UNREACHABLE_BASE).
    UnreachableRoots,
    /// Objects only retained by detached DOM nodes.
    RetainedByDetachedDom,
    /// Objects only retained by DevTools console references.
    RetainedByConsole,
    /// Objects only retained by event handlers.
    RetainedByEventHandlers,
    /// Objects attributed to a specific native context.
    NativeContext(NativeContextId),
    /// Objects shared across multiple native contexts.
    SharedContext,
    /// Objects not attributed to any native context.
    UnattributedContext,
}

impl SummaryFilterMode {
    fn label(self, snap: &HeapSnapshot) -> String {
        match self {
            Self::All => "All objects".to_string(),
            Self::Unreachable => "Unreachable (all)".to_string(),
            Self::UnreachableRoots => "Unreachable (roots only)".to_string(),
            Self::RetainedByDetachedDom => "Retained by detached DOM".to_string(),
            Self::RetainedByConsole => "Retained by DevTools console".to_string(),
            Self::RetainedByEventHandlers => "Retained by event handlers".to_string(),
            Self::NativeContext(id) => {
                let ctx = &snap.native_contexts()[id.0 as usize];
                snap.native_context_label(ctx.ordinal)
            }
            Self::SharedContext => "Shared (multiple contexts)".to_string(),
            Self::UnattributedContext => "Unattributed".to_string(),
        }
    }
}

// ── App ──────────────────────────────────────────────────────────────────

struct App {
    current_view: ViewType,
    input_mode: InputMode,
    // auto-incrementing ID counter for tree keys
    next_id: Cell<u64>,
    // pre-assigned IDs for summary aggregate rows
    summary_ids: Vec<NodeId>,
    // per-view tree state
    summary_state: TreeState,
    containment_state: TreeState,
    containment_root_id: NodeId,
    dominators_state: TreeState,
    dominators_root_id: NodeId,
    // live text in the "/" search prompt
    search_input: String,
    // error message shown after a failed search
    search_error: Option<String>,
    // lowercase substring filter applied to constructor names in Summary
    summary_filter: String,
    // unreachable filter mode for Summary view
    summary_filter_mode: SummaryFilterMode,
    // node whose edges are being filtered ("f" prompt)
    edge_filter_target: Option<NodeOrdinal>,
    // NodeId of the row whose edges are being filtered
    edge_filter_node_id: Option<NodeId>,
    // whether the edge filter target is from a compare snapshot
    edge_filter_is_compare: bool,
    // live text in the edge-filter prompt
    edge_filter_input: String,
    // Cached tab labels for the header, recomputed when terminal width changes.
    tab_cache: Vec<(String, ViewType)>,
    tab_cache_width: usize,
    // class aggregates sorted by retained size (drives the Summary view)
    sorted_aggregates: Vec<AggregateInfo>,
    // totals used for percentage columns in Summary
    summary_total_shallow: f64,
    summary_total_retained: f64,
    // root retained size used for percentage columns in Containment/Retainers
    heap_total: f64,
    // flattened rows for the current frame
    cached_rows: Vec<FlatRow>,
    // whether the flattened row cache needs to be rebuilt
    rows_dirty: bool,
    // NodeId at the cursor when rows were last dirtied — used to restore
    // cursor position after rebuild so expanding doesn't move the selection.
    cursor_anchor: Option<NodeId>,
    // diff view
    diff: DiffViewState,
    // native contexts
    contexts_ids: Vec<NodeId>,
    contexts_state: TreeState,
    // cached reachable sizes (computed on demand, persisted to disk)
    reachable_sizes: FxHashMap<NodeOrdinal, f64>,
    // ordinals currently being computed on background threads
    reachable_pending: FxHashSet<NodeOrdinal>,
    // resolved chrome extension names (extension_id -> name)
    extension_names: FxHashMap<String, String>,
    // extension IDs currently being looked up or already resolved
    extension_pending: FxHashSet<String>,
    // send work to the thread pool
    work_tx: mpsc::Sender<WorkItem>,
    // receive completed results from the thread pool
    result_rx: mpsc::Receiver<WorkResult>,
    // history of visited objects (most recent last, no consecutive duplicates)
    history: Vec<NodeOrdinal>,
    history_ids: Vec<NodeId>,
    history_state: TreeState,
    help_state: ScrollState,
    statistics_state: ScrollState,
    timeline_state: ScrollState,
    // retainers view
    retainers: RetainersViewState,
    // filter overlay state
    filter_overlay_items: Vec<FilterOverlayItem>,
    filter_overlay_cursor: usize,
    filter_overlay_scroll: usize,
}

// Core App methods used across multiple submodules.
impl App {
    #[cfg(test)]
    pub(super) fn new(
        snap: &HeapSnapshot,
        compare_snapshots: Vec<(String, HeapSnapshot)>,
        work_tx: mpsc::Sender<WorkItem>,
        result_rx: mpsc::Receiver<WorkResult>,
    ) -> Self {
        let mut sorted: Vec<AggregateInfo> = snap.aggregates_with_filter().into_values().collect();
        sorted.sort_by(|a, b| {
            b.max_ret
                .partial_cmp(&a.max_ret)
                .unwrap()
                .then(a.first_seen.cmp(&b.first_seen))
        });
        Self::new_with_aggregates(snap, sorted, compare_snapshots, work_tx, result_rx)
    }

    pub(super) fn new_with_aggregates(
        snap: &HeapSnapshot,
        sorted: Vec<AggregateInfo>,
        compare_snapshots: Vec<(String, HeapSnapshot)>,
        work_tx: mpsc::Sender<WorkItem>,
        result_rx: mpsc::Receiver<WorkResult>,
    ) -> Self {
        let summary_total_shallow: f64 = sorted.iter().map(|e| e.self_size).sum();
        let summary_total_retained: f64 = sorted.iter().map(|e| e.max_ret).sum();
        let heap_total = snap.get_statistics().total;

        let next_id = Cell::new(0u64);

        // Pre-assign stable IDs for summary aggregate rows
        let summary_ids: Vec<NodeId> = (0..sorted.len()).map(|_| mint_id(&next_id)).collect();

        // Pre-populate containment root children
        let containment_root_id = mint_id(&next_id);
        let mut containment_state = TreeState::new();
        let root_key = ChildrenKey::Edges(containment_root_id, snap.synthetic_root_ordinal());
        let root_children = compute_children(
            &root_key,
            containment_root_id,
            snap,
            &sorted,
            &containment_state.edge_windows,
            &containment_state.class_member_windows,
            &containment_state.edge_filters,
            "",
            None,
            SummaryFilterMode::All,
            &next_id,
        );
        containment_state
            .children_map
            .insert(root_key, root_children);

        // Pre-populate dominators root
        let dominators_root_id = mint_id(&next_id);
        let mut dominators_state = TreeState::new();
        let dom_root_key = ChildrenKey::DominatedChildren(snap.gc_roots_ordinal());
        let dom_root_children = compute_children(
            &dom_root_key,
            dominators_root_id,
            snap,
            &sorted,
            &dominators_state.edge_windows,
            &dominators_state.class_member_windows,
            &dominators_state.edge_filters,
            "",
            None,
            SummaryFilterMode::All,
            &next_id,
        );
        dominators_state
            .children_map
            .insert(dom_root_key, dom_root_children);

        // Compute diffs for all compare snapshots
        let mut diff_view = DiffViewState::new();
        diff_view.has_diff = !compare_snapshots.is_empty();
        for (name, s2) in compare_snapshots {
            let diffs = diff::compute_diff(snap, &s2);
            let ids: Vec<NodeId> = (0..diffs.len()).map(|_| mint_id(&next_id)).collect();
            diff_view.all_diffs.push((diffs, ids));
            let display_name = std::path::Path::new(&name)
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| name.clone());
            diff_view.compare_names.push(display_name);
            diff_view.compare_snapshots.push(s2);
        }
        // Set active diff from index 0
        if !diff_view.all_diffs.is_empty() {
            let (diffs, ids) = diff_view.all_diffs[0].clone();
            diff_view.sorted_diffs = diffs;
            diff_view.diff_ids = ids;
        }

        let contexts_ids: Vec<NodeId> = snap
            .native_contexts()
            .iter()
            .map(|_| mint_id(&next_id))
            .collect();

        App {
            current_view: ViewType::Summary,
            input_mode: InputMode::Normal,
            next_id,
            summary_ids,
            summary_state: TreeState::new(),
            containment_state,
            containment_root_id,
            dominators_state,
            dominators_root_id,
            search_input: String::new(),
            search_error: None,
            summary_filter: String::new(),
            summary_filter_mode: SummaryFilterMode::All,
            tab_cache: Vec::new(),
            tab_cache_width: 0,
            edge_filter_target: None,
            edge_filter_node_id: None,
            edge_filter_is_compare: false,
            edge_filter_input: String::new(),
            sorted_aggregates: sorted,
            summary_total_shallow,
            summary_total_retained,
            heap_total,
            cached_rows: Vec::new(),
            rows_dirty: true,
            cursor_anchor: None,
            diff: diff_view,
            contexts_ids,
            contexts_state: TreeState::new(),
            reachable_sizes: FxHashMap::default(),
            reachable_pending: FxHashSet::default(),
            extension_names: FxHashMap::default(),
            extension_pending: FxHashSet::default(),
            work_tx,
            result_rx,
            history: Vec::new(),
            history_ids: Vec::new(),
            history_state: TreeState::new(),
            help_state: ScrollState::new(),
            statistics_state: ScrollState::new(),
            timeline_state: ScrollState::new(),
            retainers: RetainersViewState::new(),
            filter_overlay_items: Vec::new(),
            filter_overlay_cursor: 0,
            filter_overlay_scroll: 0,
        }
    }

    fn current_tree_state(&self) -> &TreeState {
        match self.current_view {
            ViewType::Summary => &self.summary_state,
            ViewType::Containment => &self.containment_state,
            ViewType::Dominators => &self.dominators_state,
            ViewType::Retainers => &self.retainers.tree_state,
            ViewType::Diff => &self.diff.tree_state,
            ViewType::Contexts => &self.contexts_state,
            ViewType::History => &self.history_state,
            ViewType::Help | ViewType::Statistics | ViewType::Timeline => {
                panic!("scroll-only view has no TreeState")
            }
        }
    }

    fn current_tree_state_mut(&mut self) -> &mut TreeState {
        match self.current_view {
            ViewType::Summary => &mut self.summary_state,
            ViewType::Containment => &mut self.containment_state,
            ViewType::Dominators => &mut self.dominators_state,
            ViewType::Retainers => &mut self.retainers.tree_state,
            ViewType::Diff => &mut self.diff.tree_state,
            ViewType::Contexts => &mut self.contexts_state,
            ViewType::History => &mut self.history_state,
            ViewType::Help | ViewType::Statistics | ViewType::Timeline => {
                panic!("scroll-only view has no TreeState")
            }
        }
    }

    fn current_scroll_state(&self) -> &ScrollState {
        match self.current_view {
            ViewType::Help => &self.help_state,
            ViewType::Statistics => &self.statistics_state,
            ViewType::Timeline => &self.timeline_state,
            _ => panic!("tree view has no ScrollState"),
        }
    }

    fn current_scroll_state_mut(&mut self) -> &mut ScrollState {
        match self.current_view {
            ViewType::Help => &mut self.help_state,
            ViewType::Statistics => &mut self.statistics_state,
            ViewType::Timeline => &mut self.timeline_state,
            _ => panic!("tree view has no ScrollState"),
        }
    }

    fn retainer_path_filter(&self, node_id: NodeId) -> Option<&FxHashSet<usize>> {
        if self.retainers.unfiltered_nodes.contains(&node_id)
            || self.retainers.gc_root_path_edges.is_empty()
        {
            None
        } else {
            Some(&self.retainers.gc_root_path_edges)
        }
    }

    fn set_view(&mut self, view: ViewType, snap: &HeapSnapshot) {
        self.current_view = view;
        if view == ViewType::Contexts {
            self.queue_contexts_reachable(snap);
            self.queue_extension_name_lookups(snap);
        }
    }

    fn queue_extension_name_lookups(&mut self, snap: &HeapSnapshot) {
        for ctx in snap.native_contexts() {
            let ord = ctx.ordinal;
            if let Some(url) = snap.native_context_url(ord) {
                if let Some(ext_id) = url
                    .strip_prefix("chrome-extension://")
                    .and_then(|s| s.split('/').next())
                {
                    if !self.extension_names.contains_key(ext_id)
                        && !self.extension_pending.contains(ext_id)
                    {
                        self.extension_pending.insert(ext_id.to_string());
                        let _ = self
                            .work_tx
                            .send(WorkItem::ExtensionName(ext_id.to_string()));
                    }
                }
            }
        }
    }

    fn queue_contexts_reachable(&mut self, snap: &HeapSnapshot) {
        let ordinals: Vec<NodeOrdinal> = snap
            .native_contexts()
            .iter()
            .map(|ctx| ctx.ordinal)
            .collect();
        for ordinal in ordinals {
            self.queue_reachable(ordinal);
        }
    }

    fn row_name_content_width(row: &FlatRow) -> usize {
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
        display_width(&format!("{indent}{marker}{}", row.render.label))
    }

    fn clamp_horizontal_scroll(&mut self, name_col_width: usize) -> usize {
        let max_content_width = self
            .cached_rows
            .iter()
            .map(Self::row_name_content_width)
            .max()
            .unwrap_or(0);
        let max_scroll = max_content_width.saturating_sub(name_col_width);
        let state = self.current_tree_state_mut();
        if state.horizontal_scroll > max_scroll {
            state.horizontal_scroll = max_scroll;
        }
        state.horizontal_scroll
    }

    fn set_summary_filter(&mut self, mode: SummaryFilterMode, snap: &HeapSnapshot) {
        self.summary_filter_mode = mode;
        let aggregates = match mode {
            SummaryFilterMode::All => snap.aggregates_with_filter(),
            SummaryFilterMode::Unreachable => snap.unreachable_aggregates(),
            SummaryFilterMode::UnreachableRoots => snap.unreachable_root_aggregates(),
            SummaryFilterMode::RetainedByDetachedDom => snap.retained_by_detached_dom(),
            SummaryFilterMode::RetainedByConsole => snap.retained_by_console(),
            SummaryFilterMode::RetainedByEventHandlers => snap.retained_by_event_handlers(),
            SummaryFilterMode::NativeContext(id) => snap.aggregates_for_native_context(id),
            SummaryFilterMode::SharedContext => snap.aggregates_for_shared_context(),
            SummaryFilterMode::UnattributedContext => snap.aggregates_for_unattributed_context(),
        };
        let mut sorted: Vec<AggregateInfo> = aggregates.into_values().collect();
        sorted.sort_by(|a, b| {
            b.max_ret
                .partial_cmp(&a.max_ret)
                .unwrap()
                .then(a.first_seen.cmp(&b.first_seen))
        });
        let next_id = &self.next_id;
        self.summary_ids = (0..sorted.len()).map(|_| mint_id(next_id)).collect();
        self.summary_total_shallow = sorted.iter().map(|e| e.self_size).sum();
        self.summary_total_retained = sorted.iter().map(|e| e.max_ret).sum();
        self.sorted_aggregates = sorted;
        self.summary_state = TreeState::new();
        self.mark_rows_dirty();
    }

    fn open_filter_overlay(&mut self, snap: &HeapSnapshot) {
        let mut items = Vec::new();

        // Static filter modes
        for mode in [
            SummaryFilterMode::All,
            SummaryFilterMode::Unreachable,
            SummaryFilterMode::UnreachableRoots,
            SummaryFilterMode::RetainedByDetachedDom,
            SummaryFilterMode::RetainedByConsole,
            SummaryFilterMode::RetainedByEventHandlers,
        ] {
            items.push(FilterOverlayItem::Filter {
                label: mode.label(snap),
                mode,
            });
        }

        // Native contexts
        if !snap.native_contexts().is_empty() {
            items.push(FilterOverlayItem::Header("Native contexts".to_string()));
            for (idx, ctx) in snap.native_contexts().iter().enumerate() {
                let mut label = snap.native_context_label(ctx.ordinal);
                if let Some(url) = snap.native_context_url(ctx.ordinal) {
                    if let Some(ext_id) = url
                        .strip_prefix("chrome-extension://")
                        .and_then(|s| s.split('/').next())
                    {
                        if let Some(name) = self.extension_names.get(ext_id) {
                            label = label.replace(url, &format!("{name} ({ext_id})"));
                        }
                    }
                }
                items.push(FilterOverlayItem::Filter {
                    label,
                    mode: SummaryFilterMode::NativeContext(NativeContextId(idx as u32)),
                });
            }
            items.push(FilterOverlayItem::Filter {
                label: "Shared (multiple contexts)".to_string(),
                mode: SummaryFilterMode::SharedContext,
            });
            items.push(FilterOverlayItem::Filter {
                label: "Unattributed".to_string(),
                mode: SummaryFilterMode::UnattributedContext,
            });
        }

        // Pre-select current mode
        let current_idx = items
            .iter()
            .position(|item| matches!(item, FilterOverlayItem::Filter { mode, .. } if *mode == self.summary_filter_mode))
            .unwrap_or(0);

        self.filter_overlay_items = items;
        self.filter_overlay_cursor = current_idx;
        self.filter_overlay_scroll = 0;
        self.input_mode = InputMode::FilterOverlay;
    }

    fn current_row(&self) -> Option<&FlatRow> {
        self.cached_rows.get(self.current_tree_state().cursor)
    }

    fn subtree_end_index(&self, start_idx: usize) -> usize {
        let depth = self.cached_rows[start_idx].nav.depth;
        let mut end_idx = start_idx;
        for idx in start_idx + 1..self.cached_rows.len() {
            if self.cached_rows[idx].nav.depth <= depth {
                break;
            }
            end_idx = idx;
        }
        end_idx
    }

    /// Returns the effective member filter for a class members group.
    /// If the group name itself matches the summary filter, members are
    /// shown unfiltered (empty string). Otherwise the summary filter is
    /// applied to individual member names.
    fn member_filter_for(&self, agg_idx: usize) -> &str {
        let filter = &self.summary_filter;
        if filter.is_empty() {
            return "";
        }
        let agg = &self.sorted_aggregates[agg_idx];
        if contains_ignore_case(&agg.name, filter) {
            ""
        } else {
            filter
        }
    }

    fn is_scroll_view(&self) -> bool {
        matches!(
            self.current_view,
            ViewType::Help | ViewType::Statistics | ViewType::Timeline
        )
    }

    fn mark_rows_dirty(&mut self) {
        if self.is_scroll_view() {
            return;
        }
        if !self.rows_dirty {
            // First dirty — snapshot the NodeId at the cursor so we can
            // restore the cursor position after the tree is rebuilt.
            let cursor = self.current_tree_state().cursor;
            self.cursor_anchor = self.cached_rows.get(cursor).map(|r| r.nav.id);
        }
        self.rows_dirty = true;
    }

    fn rebuild_rows(&mut self, snap: &HeapSnapshot) {
        let anchor = self.cursor_anchor.take();
        self.cached_rows = self.flatten_tree(snap);
        self.rows_dirty = false;

        if self.is_scroll_view() {
            return;
        }

        // Try to restore the cursor to the same node it was on before.
        if let Some(target_id) = anchor {
            if let Some(pos) = self.cached_rows.iter().position(|r| r.nav.id == target_id) {
                self.current_tree_state_mut().cursor = pos;
            }
        }
    }

    fn ensure_rows(&mut self, snap: &HeapSnapshot) {
        if self.rows_dirty {
            self.rebuild_rows(snap);
        }
    }
}

// ── Entry point ──────────────────────────────────────────────────────────

pub fn run(
    snap_path: &str,
    snap: HeapSnapshot,
    compare: Vec<(String, HeapSnapshot)>,
) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    let raw_path = Path::new(snap_path);
    let path = std::fs::canonicalize(raw_path).unwrap_or_else(|_| raw_path.to_path_buf());
    let snap = Arc::new(snap);

    // Spawn thread pool for background computations.
    let (work_tx, work_rx) = mpsc::channel::<WorkItem>();
    let (result_tx, result_rx) = mpsc::channel::<WorkResult>();
    let work_rx = Arc::new(std::sync::Mutex::new(work_rx));
    for _ in 0..4 {
        let work_rx = Arc::clone(&work_rx);
        let result_tx = result_tx.clone();
        let snap = Arc::clone(&snap);
        std::thread::spawn(move || {
            while let Ok(item) = work_rx.lock().unwrap().recv() {
                match item {
                    WorkItem::ReachableSize(ordinal) => {
                        let info = snap.reachable_size(&[ordinal]);
                        let _ = result_tx.send(WorkResult::ReachableSize {
                            ordinal,
                            size: info.size,
                        });
                    }
                    WorkItem::RetainerPlan(request) => {
                        let plan = plan_gc_root_retainer_paths(
                            &snap,
                            request.ordinal,
                            RetainerAutoExpandLimits {
                                max_depth: RETAINER_AUTO_EXPAND_DEPTH,
                                max_nodes: RETAINER_AUTO_EXPAND_NODES,
                            },
                        );
                        let _ = result_tx.send(WorkResult::RetainerPlan { request, plan });
                    }
                    WorkItem::ExtensionName(extension_id) => {
                        let name = crate::resolve_chrome_extension_name(&extension_id);
                        let _ = result_tx.send(WorkResult::ExtensionName { extension_id, name });
                    }
                }
            }
        });
    }

    // Show loading screen while computing aggregates
    terminal.draw(|frame| {
        let area = frame.area();
        frame.render_widget(
            ratatui::widgets::Paragraph::new("Computing aggregates..."),
            area,
        );
    })?;
    let aggregates = snap.aggregates_with_filter();
    let mut sorted: Vec<AggregateInfo> = aggregates.into_values().collect();
    sorted.sort_by(|a, b| {
        b.max_ret
            .partial_cmp(&a.max_ret)
            .unwrap()
            .then(a.first_seen.cmp(&b.first_seen))
    });

    let mut app = App::new_with_aggregates(&snap, sorted, compare, work_tx, result_rx);

    // Restore history and reachable sizes from disk
    let (restored, reachable) = load_history(&path, &snap);
    for ord in &restored {
        app.push_history(*ord);
    }
    app.reachable_sizes = reachable;
    app.extension_names = load_extension_names();

    let mut needs_redraw = true;

    loop {
        // Background workers can complete while the UI is idle.
        needs_redraw |= app.drain_results(&snap);

        if needs_redraw {
            terminal.draw(|f| app.render(f, &snap))?;
            needs_redraw = false;
        }

        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) => {
                    if app.handle_key(key, &snap) {
                        break;
                    }
                    needs_redraw = true;
                }
                Event::Resize(_, _) => {
                    needs_redraw = true;
                }
                _ => {}
            }
        }
    }

    // Persist history and reachable sizes to disk
    save_history(&path, &app.history, &app.reachable_sizes, &snap);
    save_extension_names(&app.extension_names);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
