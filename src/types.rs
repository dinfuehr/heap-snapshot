#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Distance(pub u32);

impl Distance {
    /// Sentinel for nodes not yet visited during BFS.
    pub const NONE: Distance = Distance(u32::MAX);
    /// Base distance for unreachable nodes.  Unreachable roots get this value,
    /// their children get UNREACHABLE_BASE + 1, +2, etc.
    pub const UNREACHABLE_BASE: Distance = Distance(u32::MAX / 2);

    pub fn is_reachable(self) -> bool {
        self < Self::UNREACHABLE_BASE
    }

    pub fn is_unreachable(self) -> bool {
        self >= Self::UNREACHABLE_BASE
    }

    pub fn is_unreachable_root(self) -> bool {
        self == Self::UNREACHABLE_BASE
    }
}

impl std::fmt::Display for Distance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if *self >= Self::UNREACHABLE_BASE {
            let offset = self.0 - Self::UNREACHABLE_BASE.0;
            if offset == 0 {
                write!(f, "U")
            } else {
                write!(f, "U+{offset}")
            }
        } else {
            write!(f, "{}", self.0)
        }
    }
}

/// Internal index of a node within one loaded heap snapshot.
///
/// This is an array-position handle used throughout snapshot analysis code for
/// fast access into flat `nodes`/`edges` storage. It is only meaningful within
/// a single `HeapSnapshot` instance and should not be treated as a stable
/// object identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeOrdinal(pub usize);

/// Heap snapshot object ID from the snapshot's `id` field.
///
/// This is the external identifier shown to users as `@<id>` and used by CLI
/// and MCP APIs. These IDs increase as objects are assigned snapshot IDs, and
/// the same live object keeps the same `NodeId` across multiple snapshots.
/// Convert between `NodeId` and `NodeOrdinal` with
/// `HeapSnapshot::node_for_snapshot_object_id` and `HeapSnapshot::node_id`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub struct RawHeapSnapshot {
    pub snapshot: SnapshotHeader,
    pub nodes: Vec<u32>,
    pub edges: Vec<u32>,
    pub strings: Vec<String>,
    pub locations: Vec<u32>,
    /// Flat array of trace function info entries (6 fields each).
    pub trace_function_infos: Vec<u32>,
    /// Flattened trace tree: trace_node_id -> parent trace_node_id.
    pub trace_tree_parents: Vec<u32>,
    /// Flattened trace tree: trace_node_id -> function_info index.
    pub trace_tree_func_idxs: Vec<u32>,
    /// Samples: flat array of [timestamp_us, last_assigned_id] pairs.
    pub samples: Vec<u32>,
}

pub struct SnapshotHeader {
    pub meta: SnapshotMeta,
    pub node_count: usize,
    pub edge_count: usize,
    pub trace_function_count: usize,
    pub root_index: Option<usize>,
    pub extra_native_bytes: Option<u64>,
}

pub struct SnapshotMeta {
    pub node_fields: Vec<String>,
    pub node_type_enum: Vec<String>,
    pub edge_fields: Vec<String>,
    pub edge_type_enum: Vec<String>,
    pub location_fields: Vec<String>,
    pub sample_fields: Vec<String>,
    pub trace_function_info_fields: Vec<String>,
    pub trace_node_fields: Vec<String>,
}

pub struct Statistics {
    pub total: u64,
    pub native_total: u64,
    pub typed_arrays: u64,
    pub v8heap_total: u64,
    pub code: u64,
    pub js_arrays: u64,
    pub strings: u64,
    pub system: u64,
    pub extra_native_bytes: u64,
    pub unreachable_count: u32,
    pub unreachable_size: u64,
}

pub struct DuplicateStringInfo {
    pub value: String,
    pub count: u32,
    pub instance_size: u64,
    pub total_size: u64,
}

impl DuplicateStringInfo {
    /// Bytes wasted by duplication: total minus one instance.
    pub fn wasted_size(&self) -> u64 {
        self.total_size - self.instance_size
    }
}

pub struct AggregateInfo {
    pub count: u32,
    pub distance: Distance,
    pub self_size: u64,
    pub max_ret: u64,
    pub name: String,
    pub first_seen: u32,
    pub node_ordinals: Vec<NodeOrdinal>,
}

pub type AggregateMap = Vec<AggregateInfo>;
