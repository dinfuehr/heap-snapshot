use std::cell::Cell;
use std::rc::Rc;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::print::retainers::RetainerAutoExpandPlan;
use crate::types::{Distance, NodeOrdinal};

use super::EDGE_PAGE_SIZE;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum ViewType {
    Summary,
    Containment,
    Dominators,
    Retainers,
    Diff,
    Contexts,
    History,
    Statistics,
    Help,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum InputMode {
    Normal,
    Search,
    EdgeFilter,
}

// Unique identity for a row in the tree. Auto-incremented, Copy-cheap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct NodeId(pub(super) u64);

pub(super) fn mint_id(counter: &Cell<u64>) -> NodeId {
    let id = counter.get();
    counter.set(id + 1);
    NodeId(id)
}

// One visible row in the flattened tree, ready for rendering.
// The tree is flattened into a Vec<FlatRow> each frame so the renderer
// can index by screen position and the cursor is just a row index.
pub(super) enum FlatRowKind {
    SummaryGroup {
        distance: Option<Distance>,
        shallow_size: f64,
        retained_size: f64,
    },
    HeapNode {
        node_ordinal: Option<NodeOrdinal>,
        distance: Option<Distance>,
        shallow_size: f64,
        retained_size: f64,
        reachable_size: Option<f64>,
        detachedness: Option<u8>,
    },
    DiffGroup {
        new_count: u32,
        deleted_count: u32,
        alloc_size: f64,
        freed_size: f64,
    },
    DiffObject {
        node_ordinal: Option<NodeOrdinal>,
        is_new: bool,
        size: f64,
    },
}

pub(super) struct FlatRow {
    pub(super) nav: FlatRowNav,
    pub(super) render: FlatRowRender,
}

pub(super) struct FlatRowNav {
    // unique auto-incremented identity — used for expand/collapse tracking
    pub(super) id: NodeId,
    // immediate parent row index in the current flattened list
    pub(super) parent_row: Option<usize>,
    // nesting level (0 = top-level)
    pub(super) depth: usize,
    // whether this row can be expanded to show children
    pub(super) has_children: bool,
    // whether the row is currently expanded
    pub(super) is_expanded: bool,
    // key used to compute/cache children when expanding (None for leaves)
    pub(super) children_key: Option<ChildrenKey>,
}

pub(super) struct FlatRowRender {
    // display text shown in the name column
    pub(super) label: Rc<str>,
    pub(super) kind: FlatRowKind,
    // true when this row represents a weak-reference retainer
    pub(super) is_weak: bool,
    // true when this node is directly retained by (GC roots)
    pub(super) is_root_holder: bool,
}

impl FlatRow {
    pub(super) fn node_ordinal(&self) -> Option<NodeOrdinal> {
        match &self.render.kind {
            FlatRowKind::HeapNode { node_ordinal, .. } => *node_ordinal,
            FlatRowKind::DiffObject { node_ordinal, .. } => *node_ordinal,
            _ => None,
        }
    }
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub(super) enum ChildrenKey {
    ClassMembers(usize),
    /// Edge children for a specific row.  Keyed by the owning row's unique
    /// `NodeId` so each occurrence of the same ordinal gets its own children
    /// — prevents infinite recursion when a node references itself.
    Edges(NodeId, NodeOrdinal),
    CompareEdges(NodeId, NodeOrdinal),
    /// Retainer children for a specific row.  Keyed by the owning row's
    /// unique `NodeId` so each occurrence of the same ordinal gets its own
    /// children — keeps the tree a proper tree with no shared subtrees.
    /// The `NodeOrdinal` is carried so `compute_children` knows which node
    /// to compute retainers for when children aren't pre-built.
    Retainers(NodeId, NodeOrdinal),
    DominatedChildren(NodeOrdinal),
    DiffMembers(usize),
}

pub(super) struct ChildNode {
    // unique identity assigned at creation time
    pub(super) id: NodeId,
    // display text (e.g. "prop :: Object @123")
    pub(super) label: Rc<str>,
    // BFS distance from GC roots (None for paging status rows)
    pub(super) distance: Option<Distance>,
    // own size in bytes
    pub(super) shallow_size: f64,
    // size kept alive exclusively by this node
    pub(super) retained_size: f64,
    // index into HeapSnapshot node arrays (None for paging status rows)
    pub(super) node_ordinal: Option<NodeOrdinal>,
    // whether this node can be expanded
    pub(super) has_children: bool,
    // key to compute/cache children on expand
    pub(super) children_key: Option<ChildrenKey>,
    // true when this child was reached via a weak edge
    pub(super) is_weak: bool,
    // true when this node is directly retained by (GC roots)
    pub(super) is_root_holder: bool,
}

// Per-node edge window: which slice of (filtered) edges to show.
#[derive(Clone, Copy)]
pub(super) struct EdgeWindow {
    pub(super) start: usize,
    pub(super) count: usize,
}

impl Default for EdgeWindow {
    fn default() -> Self {
        Self {
            start: 0,
            count: EDGE_PAGE_SIZE,
        }
    }
}

// UI state for a single tree view (Summary, Containment, or Retainers).
pub(super) struct TreeState {
    // set of expanded tree path keys
    pub(super) expanded: FxHashSet<NodeId>,
    // lazily computed and cached children for each expandable node
    pub(super) children_map: FxHashMap<ChildrenKey, Vec<ChildNode>>,
    // node ordinal -> which slice of filtered edges to display
    pub(super) edge_windows: FxHashMap<NodeId, EdgeWindow>,
    // aggregate index -> paging window for filtered class members
    pub(super) class_member_windows: FxHashMap<usize, EdgeWindow>,
    // node ordinal -> lowercase substring filter for edge labels
    pub(super) edge_filters: FxHashMap<NodeOrdinal, String>,
    // index of the selected row in the flattened tree
    pub(super) cursor: usize,
    // first visible row index for scrolling
    pub(super) scroll_offset: usize,
    // horizontal scroll within the name/object column
    pub(super) horizontal_scroll: usize,
    // last rendered viewport height for this view
    pub(super) page_height: usize,
}

impl TreeState {
    pub(super) fn new() -> Self {
        Self {
            expanded: FxHashSet::default(),
            children_map: FxHashMap::default(),
            edge_windows: FxHashMap::default(),
            class_member_windows: FxHashMap::default(),
            edge_filters: FxHashMap::default(),
            cursor: 0,
            scroll_offset: 0,
            horizontal_scroll: 0,
            page_height: 1,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) enum RetainerPlanKind {
    Target,
    /// Expand GC-root paths from a specific retainer node.
    /// Carries the children key of the row the user pressed `k` on,
    /// so the result is stored under the correct key (which may be
    /// `RetainerPath(id)` for auto-expanded nodes).
    Subtree(ChildrenKey),
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct PendingRetainerPlan {
    pub(super) ordinal: NodeOrdinal,
    pub(super) kind: RetainerPlanKind,
}

pub(super) enum WorkItem {
    ReachableSize(NodeOrdinal),
    RetainerPlan(PendingRetainerPlan),
    ExtensionName(String),
}

pub(super) enum WorkResult {
    ReachableSize {
        ordinal: NodeOrdinal,
        size: f64,
    },
    RetainerPlan {
        request: PendingRetainerPlan,
        plan: RetainerAutoExpandPlan,
    },
    ExtensionName {
        extension_id: String,
        name: Option<String>,
    },
}

#[derive(Clone)]
pub(super) enum PagedChildrenParent {
    Edges {
        id: NodeId,
        ordinal: NodeOrdinal,
        is_compare: bool,
    },
    Retainers {
        id: NodeId,
        ordinal: NodeOrdinal,
    },
    ClassMembers {
        agg_idx: usize,
    },
}

use crate::print::diff::ClassDiff;
use crate::snapshot::HeapSnapshot;

/// Groups all diff-view related state into a single sub-struct.
pub(super) struct DiffViewState {
    pub(super) has_diff: bool,
    pub(super) sorted_diffs: Vec<ClassDiff>,
    pub(super) diff_ids: Vec<NodeId>,
    pub(super) tree_state: TreeState,
    pub(super) filter: String,
    pub(super) all_diffs: Vec<(Vec<ClassDiff>, Vec<NodeId>)>,
    pub(super) compare_snapshots: Vec<HeapSnapshot>,
    pub(super) compare_names: Vec<String>,
    pub(super) current_idx: usize,
}

impl DiffViewState {
    pub(super) fn new() -> Self {
        Self {
            has_diff: false,
            sorted_diffs: Vec::new(),
            diff_ids: Vec::new(),
            tree_state: TreeState::new(),
            filter: String::new(),
            all_diffs: Vec::new(),
            compare_snapshots: Vec::new(),
            compare_names: Vec::new(),
            current_idx: 0,
        }
    }
}

/// Groups all retainers-view related state into a single sub-struct.
pub(super) struct RetainersViewState {
    pub(super) tree_state: TreeState,
    pub(super) target: Option<NodeOrdinal>,
    /// Stable NodeId for the root retainers entry (the target node's children key).
    pub(super) root_id: Option<NodeId>,
    pub(super) gc_root_path_edges: FxHashSet<usize>,
    pub(super) unfiltered_nodes: FxHashSet<NodeId>,
    pub(super) plan_pending: Option<PendingRetainerPlan>,
    pub(super) plan_message: Option<String>,
}

impl RetainersViewState {
    pub(super) fn new() -> Self {
        Self {
            tree_state: TreeState::new(),
            target: None,
            root_id: None,
            gc_root_path_edges: FxHashSet::default(),
            unfiltered_nodes: FxHashSet::default(),
            plan_pending: None,
            plan_message: None,
        }
    }
}
