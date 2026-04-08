use rustc_hash::FxHashMap;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::types::{Distance, NodeId};

pub mod closure_leaks;
pub mod closures;
pub mod containment;
pub mod context_tree;
pub mod diff;
pub mod retainers;
pub mod scopes;
pub mod show;
pub mod show_retainers;
pub mod statistics;
pub mod strings;
pub mod summary;
pub mod timeline;

pub const EDGE_PAGE_SIZE: usize = 10;

/// Per-node edge window: which slice of edges to show.
#[derive(Clone, Copy, Debug)]
pub struct EdgeWindow {
    pub start: usize,
    pub count: usize,
}

impl Default for EdgeWindow {
    fn default() -> Self {
        Self {
            start: 0,
            count: EDGE_PAGE_SIZE,
        }
    }
}

/// Map from node ID to its edge window. Presence in the map means the node is expanded.
pub type ExpandMap = FxHashMap<NodeId, EdgeWindow>;

/// Per-group member window: which slice of members to show when a constructor group is expanded.
#[derive(Clone, Copy, Debug)]
pub struct GroupWindow {
    pub start: usize,
    pub count: usize,
}

const GROUP_PAGE_SIZE: usize = 100;

impl Default for GroupWindow {
    fn default() -> Self {
        Self {
            start: 0,
            count: GROUP_PAGE_SIZE,
        }
    }
}

/// Map from constructor name to its member window. Presence in the map means the group is expanded.
pub type GroupExpandMap = FxHashMap<String, GroupWindow>;

const COL_NAME_SUMMARY: usize = 65;
const COL_NAME_TREE: usize = 70;
const COL_DIST: usize = 10;
const COL_SHALLOW: usize = 12;
const COL_SHALLOW_PCT: usize = 5;
const COL_RETAINED: usize = 14;
const COL_RETAINED_PCT: usize = 5;

fn total_width(col_name: usize) -> usize {
    col_name + COL_DIST + COL_SHALLOW + COL_SHALLOW_PCT + COL_RETAINED + COL_RETAINED_PCT
}

pub fn format_size(bytes: f64) -> String {
    if bytes < 1024.0 {
        return format!("{} B", bytes as u64);
    }
    let kb = bytes / 1024.0;
    if kb < 1024.0 {
        return format!("{} kB", kb.round() as u64);
    }
    let mb = kb / 1024.0;
    format!("{mb:.1} MB")
}

pub(crate) fn format_count(n: u32) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Format a distance value for display.
/// Reachable nodes show their numeric distance.
/// Unreachable nodes show "U" (root) or "U+N" (N hops from an unreachable root).
pub fn format_distance(dist: Distance) -> String {
    dist.to_string()
}

fn print_tree_header(col_name: usize) {
    println!(
        "{:<w_name$}{:>w_dist$}{:>w_ss$}{:>w_rs$}",
        "Object",
        "Distance",
        "Shallow Size",
        "Retained Size",
        w_name = col_name,
        w_dist = COL_DIST,
        w_ss = COL_SHALLOW + COL_SHALLOW_PCT,
        w_rs = COL_RETAINED + COL_RETAINED_PCT,
    );
    println!(
        "{}",
        "\u{2500}" /* ─ */
            .repeat(total_width(col_name))
    );
}

fn print_data_cols(
    name_col: &str,
    dist: Distance,
    shallow: f64,
    retained: f64,
    total_shallow: f64,
    total_retained: f64,
) {
    let dist_str = format_distance(dist);
    let shallow_pct = pct_str(shallow, total_shallow);
    let retained_pct = pct_str(retained, total_retained);

    println!(
        "{}{:>w_d$}{:>w_s$}{:>w_sp$}{:>w_r$}{:>w_rp$}",
        name_col,
        dist_str,
        format_size(shallow),
        shallow_pct,
        format_size(retained),
        retained_pct,
        w_d = COL_DIST,
        w_s = COL_SHALLOW,
        w_sp = COL_SHALLOW_PCT,
        w_r = COL_RETAINED,
        w_rp = COL_RETAINED_PCT,
    );
}

pub(crate) fn pct_str(val: f64, total: f64) -> String {
    if total > 0.0 {
        format!("{}%", (val / total * 100.0).round() as u64)
    } else {
        "0%".to_string()
    }
}

pub(crate) fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

pub(crate) fn pad_str(s: &str, width: usize) -> String {
    let actual = display_width(s);
    if actual >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - actual))
    }
}

pub(crate) fn truncate_str(s: &str, max_width: usize) -> String {
    let actual = display_width(s);
    if actual <= max_width {
        return s.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    let ellipsis = "\u{2026}";
    let ellipsis_width = display_width(ellipsis);
    if max_width <= ellipsis_width {
        return ellipsis.to_string();
    }

    let target = max_width - ellipsis_width;
    let mut width = 0;
    let mut truncated = String::new();
    for c in s.chars() {
        let ch_width = UnicodeWidthChar::width(c).unwrap_or(0);
        if width + ch_width > target {
            break;
        }
        truncated.push(c);
        width += ch_width;
    }
    truncated.push_str(ellipsis);
    truncated
}

pub(crate) fn slice_str(s: &str, start_width: usize, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let mut skipped = 0;
    let mut taken = 0;
    let mut out = String::new();

    for c in s.chars() {
        let ch_width = UnicodeWidthChar::width(c).unwrap_or(0);
        if skipped + ch_width <= start_width {
            skipped += ch_width;
            continue;
        }
        if taken + ch_width > max_width {
            break;
        }
        out.push(c);
        taken += ch_width;
    }

    out
}
