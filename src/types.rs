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

/// Internal index of an edge record within one loaded heap snapshot.
///
/// Unique within a single snapshot, but only meaningful within that snapshot.
/// Pass to [`crate::snapshot::HeapSnapshot`] accessors like `edge_name`,
/// `edge_type_name`, `is_invisible_edge`, etc.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EdgeId(pub usize);

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct NodeRecord {
    pub(crate) type_id: u32,
    pub(crate) name: u32,
    pub(crate) id: u32,
    pub(crate) self_size: u32,
    pub(crate) edge_count: u32,
    pub(crate) detachedness: u32,
    pub(crate) trace_node_id: u32,
    pub(crate) first_edge: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EdgeRecord {
    pub(crate) type_id: u32,
    pub(crate) name_or_index: u32,
    pub(crate) to_node_ordinal: u32,
    pub(crate) _padding: u32,
}

const _: () = {
    assert!(std::mem::size_of::<NodeRecord>().is_power_of_two());
    assert!(std::mem::size_of::<EdgeRecord>().is_power_of_two());
};

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
    /// Number of ordinary `system / Context` objects. NativeContexts are excluded.
    pub context_count: u32,
    /// Bytes that are retained by ordinary Context objects.
    pub retained_by_context_size: u64,
    /// Bytes that are not retained by ordinary Context objects.
    pub not_retained_by_context_size: u64,
}

pub struct DuplicateStringsResult {
    pub duplicates: Vec<DuplicateStringInfo>,
    /// Number of strings skipped because they lack a `length` edge.
    pub skipped_count: u32,
    /// Total bytes of skipped strings.
    pub skipped_size: u64,
}

pub struct DuplicateStringInfo {
    pub value: String,
    pub count: u32,
    pub instance_size: u64,
    pub total_size: u64,
    /// True character length from the snapshot's `length` edge.
    pub length: u32,
    /// String hash from the snapshot's `hash` edge, when present.
    pub hash: Option<i64>,
    /// Whether the string value was truncated in the snapshot.
    pub truncated: bool,
    /// Whether the string uses two-byte (UTF-16) representation.
    pub two_byte: bool,
    /// Node IDs of all string objects in this group.
    pub node_ids: Vec<NodeId>,
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
