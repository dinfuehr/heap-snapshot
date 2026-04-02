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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeOrdinal(pub usize);

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
}

pub struct SnapshotHeader {
    pub meta: SnapshotMeta,
    pub node_count: usize,
    pub edge_count: usize,
    #[allow(dead_code)]
    pub trace_function_count: usize,
    pub root_index: Option<usize>,
    pub extra_native_bytes: Option<f64>,
}

pub struct SnapshotMeta {
    pub node_fields: Vec<String>,
    pub node_type_enum: Vec<String>,
    pub edge_fields: Vec<String>,
    pub edge_type_enum: Vec<String>,
    pub location_fields: Vec<String>,
    #[allow(dead_code)]
    pub sample_fields: Vec<String>,
    #[allow(dead_code)]
    pub trace_function_info_fields: Vec<String>,
    #[allow(dead_code)]
    pub trace_node_fields: Vec<String>,
}

pub struct Statistics {
    pub total: f64,
    pub native_total: f64,
    pub typed_arrays: f64,
    pub v8heap_total: f64,
    pub code: f64,
    pub js_arrays: f64,
    pub strings: f64,
    pub system: f64,
    pub extra_native_bytes: f64,
    pub unreachable_count: u32,
    pub unreachable_size: f64,
}

pub struct DuplicateStringInfo {
    pub value: String,
    pub count: u32,
    pub instance_size: f64,
    pub total_size: f64,
}

impl DuplicateStringInfo {
    /// Bytes wasted by duplication: total minus one instance.
    pub fn wasted_size(&self) -> f64 {
        self.total_size - self.instance_size
    }
}

pub struct AggregateInfo {
    pub count: u32,
    pub distance: Distance,
    pub self_size: f64,
    pub max_ret: f64,
    pub name: String,
    pub first_seen: u32,
    pub node_ordinals: Vec<NodeOrdinal>,
}
