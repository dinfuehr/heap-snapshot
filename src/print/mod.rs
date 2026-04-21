use rustc_hash::FxHashMap;

pub use crate::display::{display_width, pad_str, slice_str, truncate_str};
use crate::types::{Distance, NodeId};

pub mod closure_leaks;
pub mod closures;
pub mod containment;
pub mod context_tree;
pub mod diff;
pub mod retainers;
pub mod roots;
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

pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{} B", bytes);
    }
    let kb = bytes as f64 / 1024.0;
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
    shallow: u64,
    retained: u64,
    total_shallow: u64,
    total_retained: u64,
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

pub(crate) fn pct_str(val: u64, total: u64) -> String {
    if total > 0 {
        format!("{}%", (val as f64 / total as f64 * 100.0).round() as u64)
    } else {
        "0%".to_string()
    }
}

// display_width, pad_str, truncate_str, slice_str are re-exported from crate::display
