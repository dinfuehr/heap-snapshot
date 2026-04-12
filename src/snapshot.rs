// Copyright 2011 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// This file was started from Chromium DevTools' HeapSnapshot.ts
// (front_end/entrypoints/heap_snapshot_worker/HeapSnapshot.ts).

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use regex::Regex;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::types::{
    AggregateInfo, AggregateMap, DuplicateStringInfo, NodeId, NodeOrdinal, RawHeapSnapshot,
    Statistics,
};

pub const V8_STACK_ROOTS: &str = "(Stack roots)";
pub const CPPGC_STACK_ROOTS: &str = "C++ native stack roots";

use crate::types::Distance;
const SHIFT_FOR_CLASS_INDEX: u32 = 2;
const BITMASK_FOR_DOM_LINK_STATE: u32 = (1 << SHIFT_FOR_CLASS_INDEX) - 1;
const MAX_INTERFACE_NAME_LENGTH: usize = 60;
const MIN_INTERFACE_PROPERTY_COUNT: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RootKind {
    NonRoot = 0,
    SyntheticRoot = 1,
    SystemRoot = 2,
    UserRoot = 3,
}

#[derive(Clone, Debug, Default)]
pub struct SnapshotOptions {
    /// Treat weak edges as reachable when computing distances.
    /// Objects referenced only via weak edges get distance+1 of the
    /// retainer instead of being marked unreachable (U).
    pub weak_is_reachable: bool,
}

#[derive(Clone, Copy)]
pub struct SourceLocation {
    pub script_id: u32,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug)]
pub struct AllocationFrame {
    pub function_name: String,
    pub script_name: String,
    pub line: u32,
    pub column: u32,
}

/// A time interval in the allocation timeline.
#[derive(Clone, Debug)]
pub struct TimelineInterval {
    /// Timestamp in microseconds since tracking started.
    pub timestamp_us: u64,
    /// Start of the object ID range (exclusive).
    pub id_from: u64,
    /// End of the object ID range (inclusive).
    pub id_to: u64,
    /// Number of live objects allocated in this interval.
    pub count: u32,
    /// Total size of live objects allocated in this interval.
    pub size: u64,
}

pub struct ReachableInfo {
    pub size: f64,
    pub native_contexts: Vec<NodeOrdinal>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeContextKind {
    Main,
    Iframe,
    Utility,
}

impl NativeContextKind {
    fn as_str(self) -> &'static str {
        match self {
            NativeContextKind::Main => "main",
            NativeContextKind::Iframe => "iframe",
            NativeContextKind::Utility => "utility",
        }
    }

    fn sort_priority(self) -> u8 {
        match self {
            NativeContextKind::Main => 0,
            NativeContextKind::Iframe => 1,
            NativeContextKind::Utility => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeContextData {
    pub ordinal: NodeOrdinal,
    pub kind: NativeContextKind,
    pub is_extension: bool,
    pub size: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeContextAttributableSizes {
    pub native_contexts: Vec<NativeContextData>,
    pub shared: f64,
    pub unattributed: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NativeContextId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeContextBucket {
    Context(NativeContextId),
    Shared,
    Unattributed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReachClass {
    None,
    One(NativeContextId),
    Many,
}

pub struct HeapSnapshot {
    // Raw data
    nodes: Vec<u32>,
    edges: Vec<u32>,
    strings: Vec<String>,

    // Field offsets and counts
    node_field_count: usize,
    node_type_offset: usize,
    node_name_offset: usize,
    node_id_offset: usize,
    node_self_size_offset: usize,
    node_edge_count_offset: usize,
    node_detachedness_offset: i32,  // -1 if not present
    node_trace_node_id_offset: i32, // -1 if not present

    edge_fields_count: usize,
    edge_type_offset: usize,
    edge_name_offset: usize,
    edge_to_node_offset: usize,

    // Node type constants
    node_types: Vec<String>,
    node_array_type: u32,
    node_hidden_type: u32,
    node_object_type: u32,
    node_native_type: u32,
    node_string_type: u32,
    node_cons_string_type: u32,
    node_sliced_string_type: u32,
    node_code_type: u32,
    node_synthetic_type: u32,
    node_closure_type: u32,
    node_regexp_type: u32,
    node_number_type: u32,

    // Edge type constants
    edge_types: Vec<String>,
    edge_element_type: u32,
    edge_hidden_type: u32,
    edge_internal_type: u32,
    edge_shortcut_type: u32,
    edge_weak_type: u32,
    edge_invisible_type: u32,
    edge_property_type: u32,
    #[allow(dead_code)]
    edge_context_type: u32,

    // Computed data
    node_count: usize,
    edge_count: usize,
    root_node_index: usize,
    gc_roots_ordinal: usize,
    first_edge_indexes: Vec<u32>,
    node_distances: Vec<Distance>,
    retained_sizes: Vec<f64>,
    dominators_tree: Vec<u32>,
    dominated_nodes: Vec<u32>,
    first_dominated_node_index: Vec<u32>,

    // Retainers
    retaining_nodes: Vec<u32>,
    retaining_edges: Vec<u32>,
    first_retainer_index: Vec<u32>,

    // Flags (for page-owned nodes tracking)
    flags: Vec<u8>,

    // Root classification for each node ordinal.
    root_kinds: Vec<RootKind>,

    // Class index per node (separate array if detachedness offset not present)
    detachedness_and_class_index: Vec<u32>,
    use_separate_class_index: bool,

    // Location map: node_index -> SourceLocation
    location_map: FxHashMap<usize, SourceLocation>,
    // Script ID -> script name (e.g. "file.js")
    script_names: FxHashMap<u32, String>,
    location_field_count: usize,
    location_index_offset: usize,
    location_script_id_offset: usize,
    location_line_offset: usize,
    location_column_offset: usize,

    // Native contexts (ordinals of "system / NativeContext" nodes) with
    // attributable size metadata.
    native_contexts: Vec<NativeContextData>,
    // Best-effort native context owner for each node.
    node_native_context_buckets: Vec<NativeContextBucket>,
    shared_attributable_size: f64,
    unattributed_size: f64,
    // Edge names common to all NativeContext global_objects
    native_context_global_fields: FxHashSet<String>,
    // Precomputed "Vars" string per NativeContext (joined unique global + script context vars)
    native_context_vars: FxHashMap<NodeOrdinal, String>,
    // Object ordinals whose constructor type maps to JSGlobalObject / JSGlobalProxy
    js_global_objects: Vec<usize>,
    js_global_proxies: Vec<usize>,
    // Edge names common to all objects of those two kinds
    js_global_object_fields: FxHashSet<String>,
    js_global_proxy_fields: FxHashSet<String>,

    // Allocation trace data
    trace_parents: Vec<u32>,               // trace_node_id -> parent_id
    trace_func_idxs: Vec<u32>,             // trace_node_id -> function_info_index
    trace_functions: Vec<AllocationFrame>, // function_info_index -> frame
    timeline: Vec<TimelineInterval>,       // allocation timeline intervals

    // Extra native bytes from snapshot header
    extra_native_bytes: f64,

    // When true, weak edges are treated as reachable during BFS distance
    // computation.  Objects referenced only via weak edges from reachable
    // nodes get distance+1 of the retainer instead of being marked
    // unreachable (U).
    weak_is_reachable: bool,

    // Statistics (computed at init)
    statistics: Statistics,
}

impl HeapSnapshot {
    pub fn new(raw: RawHeapSnapshot) -> Self {
        Self::new_with_options(raw, SnapshotOptions::default())
    }

    pub fn new_with_options(raw: RawHeapSnapshot, options: SnapshotOptions) -> Self {
        let meta = &raw.snapshot.meta;

        // Node field offsets
        let node_type_offset = meta.node_fields.iter().position(|f| f == "type").unwrap();
        let node_name_offset = meta.node_fields.iter().position(|f| f == "name").unwrap();
        let node_id_offset = meta.node_fields.iter().position(|f| f == "id").unwrap();
        let node_self_size_offset = meta
            .node_fields
            .iter()
            .position(|f| f == "self_size")
            .unwrap();
        let node_edge_count_offset = meta
            .node_fields
            .iter()
            .position(|f| f == "edge_count")
            .unwrap();
        let node_detachedness_offset = meta
            .node_fields
            .iter()
            .position(|f| f == "detachedness")
            .map(|p| p as i32)
            .unwrap_or(-1);
        let node_trace_node_id_offset = meta
            .node_fields
            .iter()
            .position(|f| f == "trace_node_id")
            .map(|p| p as i32)
            .unwrap_or(-1);
        let node_field_count = meta.node_fields.len();

        // Node types
        let node_types = meta.node_type_enum.clone();
        let find_node_type = |name: &str| -> u32 {
            node_types
                .iter()
                .position(|t| t == name)
                .unwrap_or(usize::MAX) as u32
        };
        let node_array_type = find_node_type("array");
        let node_hidden_type = find_node_type("hidden");
        let node_object_type = find_node_type("object");
        let node_native_type = find_node_type("native");
        let node_string_type = find_node_type("string");
        let node_cons_string_type = find_node_type("concatenated string");
        let node_sliced_string_type = find_node_type("sliced string");
        let node_code_type = find_node_type("code");
        let node_synthetic_type = find_node_type("synthetic");
        let node_closure_type = find_node_type("closure");
        let node_regexp_type = find_node_type("regexp");
        let node_number_type = find_node_type("number");

        // Edge field offsets
        let edge_fields_count = meta.edge_fields.len();
        let edge_type_offset = meta.edge_fields.iter().position(|f| f == "type").unwrap();
        let edge_name_offset = meta
            .edge_fields
            .iter()
            .position(|f| f == "name_or_index")
            .unwrap();
        let edge_to_node_offset = meta
            .edge_fields
            .iter()
            .position(|f| f == "to_node")
            .unwrap();

        // Edge types (add 'invisible' like Chrome DevTools does)
        let mut edge_types = meta.edge_type_enum.clone();
        edge_types.push("invisible".to_string());
        let find_edge_type = |name: &str| -> u32 {
            edge_types
                .iter()
                .position(|t| t == name)
                .unwrap_or(usize::MAX) as u32
        };
        let edge_element_type = find_edge_type("element");
        let edge_hidden_type = find_edge_type("hidden");
        let edge_internal_type = find_edge_type("internal");
        let edge_shortcut_type = find_edge_type("shortcut");
        let edge_weak_type = find_edge_type("weak");
        let edge_invisible_type = find_edge_type("invisible");
        let edge_property_type = find_edge_type("property");
        let edge_context_type = find_edge_type("context");

        // Location fields
        let location_fields = &meta.location_fields;
        let location_index_offset = location_fields
            .iter()
            .position(|f| f == "object_index")
            .unwrap_or(0);
        let location_script_id_offset = location_fields
            .iter()
            .position(|f| f == "script_id")
            .unwrap_or(1);
        let location_line_offset = location_fields
            .iter()
            .position(|f| f == "line")
            .unwrap_or(2);
        let location_column_offset = location_fields
            .iter()
            .position(|f| f == "column")
            .unwrap_or(3);
        let location_field_count = if location_fields.is_empty() {
            4
        } else {
            location_fields.len()
        };

        let node_count = raw.nodes.len() / node_field_count;
        let edge_count = raw.edges.len() / edge_fields_count;
        let root_node_index = raw.snapshot.root_index.unwrap_or(0);
        let extra_native_bytes = raw.snapshot.extra_native_bytes.unwrap_or(0.0);

        let mut snap = HeapSnapshot {
            nodes: raw.nodes,
            edges: raw.edges,
            strings: raw.strings,
            node_field_count,
            node_type_offset,
            node_name_offset,
            node_id_offset,
            node_self_size_offset,
            node_edge_count_offset,
            node_detachedness_offset,
            node_trace_node_id_offset,
            edge_fields_count,
            edge_type_offset,
            edge_name_offset,
            edge_to_node_offset,
            node_types,
            node_array_type,
            node_hidden_type,
            node_object_type,
            node_native_type,
            node_string_type,
            node_cons_string_type,
            node_sliced_string_type,
            node_code_type,
            node_synthetic_type,
            node_closure_type,
            node_regexp_type,
            node_number_type,
            edge_types,
            edge_element_type,
            edge_hidden_type,
            edge_internal_type,
            edge_shortcut_type,
            edge_weak_type,
            edge_invisible_type,
            edge_property_type,
            edge_context_type,
            node_count,
            edge_count,
            root_node_index,
            gc_roots_ordinal: root_node_index / node_field_count, // updated after build_edge_indexes
            first_edge_indexes: vec![0u32; node_count + 1],
            node_distances: vec![Distance::NONE; node_count],
            retained_sizes: vec![0.0; node_count],
            dominators_tree: vec![0u32; node_count],
            dominated_nodes: Vec::new(),
            first_dominated_node_index: vec![0u32; node_count + 1],
            retaining_nodes: vec![0u32; edge_count],
            retaining_edges: vec![0u32; edge_count],
            first_retainer_index: vec![0u32; node_count + 1],
            flags: vec![0u8; node_count],
            root_kinds: vec![RootKind::NonRoot; node_count],
            detachedness_and_class_index: Vec::new(),
            use_separate_class_index: false,
            native_contexts: Vec::new(),
            node_native_context_buckets: vec![NativeContextBucket::Unattributed; node_count],
            shared_attributable_size: 0.0,
            unattributed_size: 0.0,
            native_context_global_fields: FxHashSet::default(),
            native_context_vars: FxHashMap::default(),
            js_global_objects: Vec::new(),
            js_global_proxies: Vec::new(),
            js_global_object_fields: FxHashSet::default(),
            js_global_proxy_fields: FxHashSet::default(),
            trace_parents: Vec::new(),
            trace_func_idxs: Vec::new(),
            trace_functions: Vec::new(),
            timeline: Vec::new(),
            location_map: FxHashMap::default(),
            script_names: FxHashMap::default(),
            location_field_count,
            location_index_offset,
            location_script_id_offset,
            location_line_offset,
            location_column_offset,
            extra_native_bytes,
            weak_is_reachable: options.weak_is_reachable,
            statistics: Statistics {
                total: 0.0,
                native_total: 0.0,
                typed_arrays: 0.0,
                v8heap_total: 0.0,
                code: 0.0,
                js_arrays: 0.0,
                strings: 0.0,
                system: 0.0,
                extra_native_bytes: 0.0,
                unreachable_count: 0,
                unreachable_size: 0.0,
            },
        };

        // Build edge indexes
        snap.build_edge_indexes();

        // Find native contexts and their common fields
        snap.find_native_contexts();
        snap.build_native_context_global_fields();
        snap.build_native_context_vars();
        snap.find_js_globals();
        snap.build_js_global_fields();

        // Find (GC roots) ordinal — must happen after edge indexes are built
        snap.gc_roots_ordinal = match snap.find_gc_roots_ordinal() {
            Some(ord) => ord,
            None => {
                let nfc = snap.node_field_count;
                let efc = snap.edge_fields_count;
                let eto = snap.edge_to_node_offset;
                let root_ord = snap.root_node_index / nfc;
                let first = snap.first_edge_indexes[root_ord] as usize;
                let last = snap.first_edge_indexes[root_ord + 1] as usize;
                let mut children = Vec::new();
                let mut ei = first;
                while ei < last {
                    let child_index = snap.edges[ei + eto] as usize;
                    let name =
                        &snap.strings[snap.nodes[child_index + snap.node_name_offset] as usize];
                    children.push(name.clone());
                    ei += efc;
                }
                panic!(
                    "Could not find (GC roots) among root node's {} children: [{}]",
                    children.len(),
                    children.join(", ")
                );
            }
        };

        // Classify root kinds for direct children of the synthetic root.
        {
            let nfc = snap.node_field_count;
            let efc = snap.edge_fields_count;
            let root_ord = snap.root_node_index / nfc;
            snap.root_kinds[root_ord] = RootKind::SyntheticRoot;
            let first = snap.first_edge_indexes[root_ord] as usize;
            let last = snap.first_edge_indexes[root_ord + 1] as usize;
            let mut ei = first;
            while ei < last {
                let child_index = snap.edges[ei + snap.edge_to_node_offset] as usize;
                let child_ordinal = child_index / nfc;
                let child_type = snap.nodes[child_index + snap.node_type_offset];
                if child_type != snap.node_synthetic_type {
                    // Non-synthetic child of the synthetic root is a user root.
                    snap.root_kinds[child_ordinal] = RootKind::UserRoot;
                } else {
                    let name =
                        &snap.strings[snap.nodes[child_index + snap.node_name_offset] as usize];
                    if name == "(Document DOM trees)" {
                        // "(Document DOM trees)" is synthetic but treated as a user root.
                        snap.root_kinds[child_ordinal] = RootKind::UserRoot;
                    } else {
                        snap.root_kinds[child_ordinal] = RootKind::SystemRoot;
                    }
                }
                ei += efc;
            }
        }

        // Build retainers
        snap.build_retainers();

        // Propagate DOM state
        snap.propagate_dom_state();

        // Calculate flags
        snap.calculate_flags();

        // NOTE: DevTools calls calculateEffectiveSizes here, which
        // transfers shallow sizes from hidden/array/ExternalStringData nodes
        // to their unique non-synthetic owner.  This makes summary view sizes
        // more meaningful by attributing internal backing stores (e.g. the
        // FixedArray behind an Array) to the owning JS object.  We skip this
        // because it only runs when user roots (NativeContexts) are present,
        // and our target snapshots don't use them.

        // Compute essential edges
        let essential_edges = snap.init_essential_edges();

        // Calculate dominators and retained sizes
        snap.calculate_dominators_and_retained_sizes(&essential_edges);

        // Build dominated nodes
        snap.build_dominated_nodes();

        // Calculate distances
        snap.calculate_distances();

        // Classify each node into a native-context bucket using direct
        // inference first, then context reachability.
        snap.compute_node_native_context_buckets();
        snap.compute_native_context_attributable_sizes();

        // Calculate depths within the unreachable subgraph
        snap.calculate_unreachable_depths();

        // Calculate object names
        snap.calculate_object_names();

        // Infer and apply interface definitions
        snap.infer_and_apply_interface_definitions();

        // Build location map
        {
            let locations = &raw.locations;
            let mut map = FxHashMap::default();
            let mut i = 0;
            while i < locations.len() {
                let node_index = locations[i + snap.location_index_offset] as usize;
                map.insert(
                    node_index,
                    SourceLocation {
                        script_id: locations[i + snap.location_script_id_offset],
                        line: locations[i + snap.location_line_offset],
                        column: locations[i + snap.location_column_offset],
                    },
                );
                i += snap.location_field_count;
            }
            snap.location_map = map;
        }

        snap.compute_script_names();

        // Build allocation trace data
        if !raw.trace_tree_parents.is_empty() {
            snap.trace_parents = raw.trace_tree_parents;
            snap.trace_func_idxs = raw.trace_tree_func_idxs;
            snap.build_trace_functions(&raw.trace_function_infos, &meta);
        }

        // Build allocation timeline from samples
        if !raw.samples.is_empty() {
            snap.build_timeline(&raw.samples, &meta);
        }

        // Calculate statistics
        snap.calculate_statistics();

        snap
    }

    fn find_native_contexts(&mut self) {
        for ordinal in 0..self.node_count {
            let ordinal = NodeOrdinal(ordinal);
            if self.is_native_context(ordinal) {
                self.native_contexts.push(NativeContextData {
                    ordinal,
                    kind: self.compute_native_context_kind(ordinal),
                    is_extension: self
                        .native_context_url(ordinal)
                        .is_some_and(|url| url.starts_with("chrome-extension://")),
                    size: 0.0,
                });
            }
        }

        // Assign earlier native-context IDs to primary page contexts before
        // less-interesting buckets like utility and extension contexts.
        self.native_contexts
            .sort_by_key(|ctx| (ctx.is_extension, ctx.kind.sort_priority(), ctx.ordinal.0));
    }

    fn compute_native_context_kind(&self, ordinal: NodeOrdinal) -> NativeContextKind {
        let is_frame = self
            .find_edge_target(ordinal, "global_object")
            .is_some_and(|go| self.node_raw_name(go).starts_with("Window"));

        if !is_frame {
            return NativeContextKind::Utility;
        }

        match self.find_edge_target(ordinal, "global_proxy_object") {
            Some(gp) if self.node_edge_count(gp) >= 10 => NativeContextKind::Main,
            _ => NativeContextKind::Iframe,
        }
    }

    fn compute_node_native_context_buckets(&mut self) {
        let mut context_index_by_ordinal = FxHashMap::default();
        for (idx, ctx) in self.native_contexts.iter().enumerate() {
            context_index_by_ordinal.insert(ctx.ordinal.0, NativeContextId(idx as u32));
        }

        let mut fixed_owner = vec![None; self.node_count];
        for ordinal in 0..self.node_count {
            fixed_owner[ordinal] =
                self.infer_direct_native_context(NodeOrdinal(ordinal), &context_index_by_ordinal);
        }

        let mut reach_owner = vec![ReachClass::None; self.node_count];
        let mut queue = VecDeque::new();
        for (ordinal, owner) in fixed_owner.iter().enumerate() {
            if owner.is_some() {
                queue.push_back(ordinal);
            }
        }

        let nfc = self.node_field_count;
        let efc = self.edge_fields_count;
        let eto = self.edge_to_node_offset;
        let etype_off = self.edge_type_offset;

        while let Some(ordinal) = queue.pop_front() {
            let current = match fixed_owner[ordinal] {
                Some(ctx_idx) => ReachClass::One(ctx_idx),
                None => reach_owner[ordinal],
            };
            if current == ReachClass::None {
                continue;
            }

            let first_edge = self.first_edge_indexes[ordinal] as usize;
            let last_edge = self.first_edge_indexes[ordinal + 1] as usize;
            let mut ei = first_edge;
            while ei < last_edge {
                let edge_type = self.edges[ei + etype_off];
                // We intentionally propagate through weak edges here. The
                // bucket answers "which native contexts can reach this object"
                // rather than "which contexts strongly retain it", and weak
                // structures such as WeakMaps still provide useful context
                // attribution signal. Shortcut edges remain excluded because
                // V8 emits them as synthetic navigation aids, not structural
                // graph edges.
                if edge_type == self.edge_shortcut_type {
                    ei += efc;
                    continue;
                }
                let child_ordinal = self.edges[ei + eto] as usize / nfc;
                if child_ordinal == ordinal || fixed_owner[child_ordinal].is_some() {
                    ei += efc;
                    continue;
                }
                let merged = Self::merge_reach_class(reach_owner[child_ordinal], current);
                if merged != reach_owner[child_ordinal] {
                    reach_owner[child_ordinal] = merged;
                    queue.push_back(child_ordinal);
                }
                ei += efc;
            }
        }

        for ordinal in 0..self.node_count {
            self.node_native_context_buckets[ordinal] = match fixed_owner[ordinal] {
                Some(id) => NativeContextBucket::Context(id),
                None => match reach_owner[ordinal] {
                    ReachClass::One(id) => NativeContextBucket::Context(id),
                    ReachClass::Many => NativeContextBucket::Shared,
                    ReachClass::None => NativeContextBucket::Unattributed,
                },
            };
        }
    }

    fn compute_native_context_attributable_sizes(&mut self) {
        let mut native_context_sizes = vec![0.0; self.native_contexts.len()];
        let mut shared_size = 0.0;
        let mut unattributed_size = 0.0;

        for ordinal in 0..self.node_count {
            let size = self.node_self_size(NodeOrdinal(ordinal)) as f64;
            match self.node_native_context_buckets[ordinal] {
                NativeContextBucket::Context(id) => {
                    native_context_sizes[id.0 as usize] += size;
                }
                NativeContextBucket::Shared => {
                    shared_size += size;
                }
                NativeContextBucket::Unattributed => {
                    unattributed_size += size;
                }
            }
        }

        for (ctx, size) in self
            .native_contexts
            .iter_mut()
            .zip(native_context_sizes.into_iter())
        {
            ctx.size = size;
        }
        self.shared_attributable_size = shared_size;
        self.unattributed_size = unattributed_size;
    }

    fn infer_direct_native_context(
        &self,
        ordinal: NodeOrdinal,
        context_index_by_ordinal: &FxHashMap<usize, NativeContextId>,
    ) -> Option<NativeContextId> {
        if self.is_native_context(ordinal) {
            return context_index_by_ordinal.get(&ordinal.0).copied();
        }

        if self.is_context(ordinal) {
            if let Some(ctx) = self.find_native_context_for_context(ordinal) {
                return context_index_by_ordinal.get(&ctx.0).copied();
            }
        }

        if let Some(ctx_idx) = self
            .find_edge_target(ordinal, "context")
            .and_then(|ctx| self.resolve_native_context_candidate(ctx, context_index_by_ordinal))
        {
            return Some(ctx_idx);
        }

        if let Some(ctx_idx) = self
            .find_edge_target(ordinal, "native_context")
            .and_then(|ctx| self.resolve_native_context_candidate(ctx, context_index_by_ordinal))
        {
            return Some(ctx_idx);
        }

        if let Some(map_ordinal) = self.find_edge_target(ordinal, "map") {
            if let Some(ctx_idx) = self
                .find_edge_target(map_ordinal, "native_context")
                .and_then(|ctx| {
                    self.resolve_native_context_candidate(ctx, context_index_by_ordinal)
                })
            {
                return Some(ctx_idx);
            }
        }

        None
    }

    fn resolve_native_context_candidate(
        &self,
        ordinal: NodeOrdinal,
        context_index_by_ordinal: &FxHashMap<usize, NativeContextId>,
    ) -> Option<NativeContextId> {
        if self.is_native_context(ordinal) {
            return context_index_by_ordinal.get(&ordinal.0).copied();
        }
        if self.is_context(ordinal) {
            return self
                .find_native_context_for_context(ordinal)
                .and_then(|ctx| context_index_by_ordinal.get(&ctx.0).copied());
        }
        None
    }

    fn merge_reach_class(current: ReachClass, incoming: ReachClass) -> ReachClass {
        match (current, incoming) {
            (ReachClass::Many, _) | (_, ReachClass::Many) => ReachClass::Many,
            (ReachClass::None, other) => other,
            (other, ReachClass::None) => other,
            (ReachClass::One(a), ReachClass::One(b)) if a == b => ReachClass::One(a),
            (ReachClass::One(_), ReachClass::One(_)) => ReachClass::Many,
        }
    }

    fn find_js_globals(&mut self) {
        for ordinal in 0..self.node_count {
            match Self::normalize_constructor_type(self.node_raw_name(NodeOrdinal(ordinal))) {
                Some("[JSGlobalObject]") => self.js_global_objects.push(ordinal),
                Some("[JSGlobalProxy]") => self.js_global_proxies.push(ordinal),
                _ => {}
            }
        }
    }

    /// Known fields on NativeContext global objects.
    ///
    /// This list contains standard JS builtins (present on all JS globals)
    /// plus standard Window API properties, methods, and event handlers from
    /// Chromium's IDL definitions.  Event handler attributes (on*) each
    /// generate three edges: the property itself plus "get on*" / "set on*".
    /// Those getter/setter variants are added programmatically in
    /// `build_native_context_global_fields` so only the bare name appears here.
    const KNOWN_GLOBAL_FIELDS: &'static [&'static str] = &[
        // ── JS builtins (present on every global object) ──
        "AggregateError",
        "Array",
        "ArrayBuffer",
        "Atomics",
        "BigInt",
        "BigInt64Array",
        "BigUint64Array",
        "Boolean",
        "DataView",
        "Date",
        "Error",
        "EvalError",
        "FinalizationRegistry",
        "Float16Array",
        "Float32Array",
        "Float64Array",
        "Function",
        "Int8Array",
        "Int16Array",
        "Int32Array",
        "Intl",
        "Iterator",
        "JSON",
        "Map",
        "Math",
        "NaN",
        "Number",
        "Object",
        "Promise",
        "Proxy",
        "RangeError",
        "ReferenceError",
        "Reflect",
        "RegExp",
        "Set",
        "SharedArrayBuffer",
        "String",
        "SuppressedError",
        "Symbol",
        "SyntaxError",
        "TypeError",
        "URIError",
        "Uint8Array",
        "Uint8ClampedArray",
        "Uint16Array",
        "Uint32Array",
        "WeakMap",
        "WeakRef",
        "WeakSet",
        "WebAssembly",
        "__proto__",
        "decodeURI",
        "decodeURIComponent",
        "encodeURI",
        "encodeURIComponent",
        "escape",
        "eval",
        "globalThis",
        "isFinite",
        "isNaN",
        "parseFloat",
        "parseInt",
        "undefined",
        "unescape",
        "Infinity",
        // V8 internals on globals
        "map",
        "properties",
        "__proto__",
        "<symbol Symbol.toStringTag>",
        // d8 shell builtins
        "console",
        "d8",
        "gc",
        "arguments",
        "global_proxy",
        "load",
        "os",
        "print",
        "printErr",
        "quit",
        "read",
        "readbuffer",
        "readline",
        "version",
        "write",
        "writeFile",
        "Realm",
        "Worker",
        "AsyncDisposableStack",
        "DisposableStack",
        // ── Window interface (from Window.idl) ──
        "window",
        "self",
        "document",
        "location",
        "customElements",
        "history",
        "navigation",
        "locationbar",
        "menubar",
        "personalbar",
        "scrollbars",
        "statusbar",
        "toolbar",
        "closed",
        "frames",
        "length",
        "top",
        "parent",
        "frameElement",
        "navigator",
        "originAgentCluster",
        "origin",
        "external",
        "screen",
        "innerWidth",
        "innerHeight",
        "scrollX",
        "pageXOffset",
        "scrollY",
        "pageYOffset",
        "visualViewport",
        "viewport",
        "screenX",
        "screenY",
        "outerWidth",
        "outerHeight",
        "devicePixelRatio",
        "event",
        "clientInformation",
        "offscreenBuffering",
        "screenLeft",
        "screenTop",
        "styleMedia",
        "credentialless",
        "fence",
        "crashReport",
        "name",
        "status",
        "opener",
        "isSecureContext",
        "crossOriginIsolated",
        "close",
        "stop",
        "focus",
        "blur",
        "open",
        "alert",
        "confirm",
        "prompt",
        "print",
        "postMessage",
        "requestAnimationFrame",
        "cancelAnimationFrame",
        "captureEvents",
        "releaseEvents",
        "getComputedStyle",
        "matchMedia",
        "moveTo",
        "moveBy",
        "resizeTo",
        "resizeBy",
        "scroll",
        "scrollTo",
        "scrollBy",
        "getSelection",
        "find",
        "webkitRequestAnimationFrame",
        "webkitCancelAnimationFrame",
        // ── UniversalGlobalScope ──
        "reportError",
        "btoa",
        "atob",
        "queueMicrotask",
        "structuredClone",
        // ── WindowOrWorkerGlobalScope ──
        "indexedDB",
        "crypto",
        "caches",
        "trustedTypes",
        "performance",
        "scheduler",
        "setTimeout",
        "clearTimeout",
        "setInterval",
        "clearInterval",
        "fetch",
        "createImageBitmap",
        // ── Module extensions on Window ──
        "sharedStorage",
        "launchQueue",
        "documentPictureInPicture",
        "cookieStore",
        "speechSynthesis",
        "requestIdleCallback",
        "cancelIdleCallback",
        "fetchLater",
        "getScreenDetails",
        "queryLocalFonts",
        "showOpenFilePicker",
        "showSaveFilePicker",
        "showDirectoryPicker",
        "webkitRequestFileSystem",
        "webkitResolveLocalFileSystemURL",
        "chrome",
        "localStorage",
        "sessionStorage",
    ];

    /// Event handler attribute names on Window.
    /// Each generates three global-object edges: `on*`, `get on*`, `set on*`.
    const KNOWN_GLOBAL_EVENT_HANDLERS: &'static [&'static str] = &[
        // GlobalEventHandlers
        "onabort",
        "onbeforeinput",
        "onbeforematch",
        "onbeforetoggle",
        "onblur",
        "oncancel",
        "oncanplay",
        "oncanplaythrough",
        "onchange",
        "onclick",
        "onclose",
        "oncommand",
        "oncontentvisibilityautostatechange",
        "oncontextlost",
        "oncontextmenu",
        "oncontextrestored",
        "oncuechange",
        "ondblclick",
        "ondrag",
        "ondragend",
        "ondragenter",
        "ondragleave",
        "ondragover",
        "ondragstart",
        "ondrop",
        "ondurationchange",
        "onemptied",
        "onended",
        "onerror",
        "onfocus",
        "onformdata",
        "oninput",
        "oninvalid",
        "onkeydown",
        "onkeypress",
        "onkeyup",
        "onload",
        "onloadeddata",
        "onloadedmetadata",
        "onloadstart",
        "onmousedown",
        "onmouseenter",
        "onmouseleave",
        "onmousemove",
        "onmouseout",
        "onmouseover",
        "onmouseup",
        "onmousewheel",
        "onpause",
        "onplay",
        "onplaying",
        "onprogress",
        "onratechange",
        "onreset",
        "onresize",
        "onscroll",
        "onscrollend",
        "onsecuritypolicyviolation",
        "onseeked",
        "onseeking",
        "onselect",
        "onslotchange",
        "onscrollsnapchange",
        "onscrollsnapchanging",
        "onstalled",
        "onsubmit",
        "onsuspend",
        "ontimeupdate",
        "ontoggle",
        "onvolumechange",
        "onwaiting",
        "onwebkitanimationend",
        "onwebkitanimationiteration",
        "onwebkitanimationstart",
        "onwebkittransitionend",
        "onwheel",
        "onauxclick",
        "ongotpointercapture",
        "onlostpointercapture",
        "onpointerdown",
        "onpointermove",
        "onpointerrawupdate",
        "onpointerup",
        "onpointercancel",
        "onpointerover",
        "onpointerout",
        "onpointerenter",
        "onpointerleave",
        "onselectstart",
        "onselectionchange",
        "onanimationcancel",
        "onanimationend",
        "onanimationiteration",
        "onanimationstart",
        "ontransitionrun",
        "ontransitionstart",
        "ontransitionend",
        "ontransitioncancel",
        "onsearch",
        // WindowEventHandlers
        "onafterprint",
        "onbeforeprint",
        "onbeforeunload",
        "onhashchange",
        "onlanguagechange",
        "onmessage",
        "onmessageerror",
        "onoffline",
        "ononline",
        "onpagehide",
        "onpageshow",
        "onpagereveal",
        "onpageswap",
        "onpopstate",
        "onrejectionhandled",
        "onstorage",
        "onunhandledrejection",
        "onunload",
        // Module event handlers
        "ondevicemotion",
        "ondeviceorientation",
        "ondeviceorientationabsolute",
        "onappinstalled",
        "onbeforeinstallprompt",
        "onbeforexrselect",
        "ongamepadconnected",
        "ongamepaddisconnected",
    ];

    fn build_native_context_global_fields(&mut self) {
        // Start with known fields.
        self.native_context_global_fields = Self::KNOWN_GLOBAL_FIELDS
            .iter()
            .map(|&s| s.to_string())
            .collect();
        // Event handlers generate property + "get on*" / "set on*" accessors.
        for &handler in Self::KNOWN_GLOBAL_EVENT_HANDLERS {
            self.native_context_global_fields
                .insert(handler.to_string());
            self.native_context_global_fields
                .insert(format!("get {handler}"));
            self.native_context_global_fields
                .insert(format!("set {handler}"));
        }
        // Also expand get/set for read/write Window attributes.
        for &attr in &[
            "clientInformation",
            "devicePixelRatio",
            "event",
            "external",
            "frames",
            "innerHeight",
            "innerWidth",
            "length",
            "location",
            "locationbar",
            "menubar",
            "name",
            "navigation",
            "offscreenBuffering",
            "opener",
            "origin",
            "outerHeight",
            "outerWidth",
            "pageXOffset",
            "pageYOffset",
            "parent",
            "performance",
            "personalbar",
            "scheduler",
            "screen",
            "screenLeft",
            "screenTop",
            "screenX",
            "screenY",
            "scrollX",
            "scrollY",
            "scrollbars",
            "self",
            "status",
            "statusbar",
            "toolbar",
            "viewport",
            "visualViewport",
        ] {
            self.native_context_global_fields
                .insert(format!("get {attr}"));
            self.native_context_global_fields
                .insert(format!("set {attr}"));
        }
        // Readonly attributes get only a getter.
        for &attr in &[
            "caches",
            "closed",
            "cookieStore",
            "crashReport",
            "credentialless",
            "crossOriginIsolated",
            "crypto",
            "customElements",
            "document",
            "documentPictureInPicture",
            "fence",
            "frameElement",
            "history",
            "indexedDB",
            "isSecureContext",
            "launchQueue",
            "localStorage",
            "navigator",
            "originAgentCluster",
            "sessionStorage",
            "sharedStorage",
            "speechSynthesis",
            "styleMedia",
            "top",
            "trustedTypes",
            "window",
        ] {
            self.native_context_global_fields
                .insert(format!("get {attr}"));
        }
    }

    fn build_native_context_vars(&mut self) {
        let contexts: Vec<NodeOrdinal> =
            self.native_contexts.iter().map(|ctx| ctx.ordinal).collect();
        for &ord in &contexts {
            let mut vars = self.native_context_global_unique_fields(ord);
            let script_vars = self.native_context_script_context_vars(ord);
            for v in script_vars {
                if !vars.contains(&v) {
                    vars.push(v);
                }
            }
            vars.sort();
            self.native_context_vars.insert(ord, vars.join(", "));
        }
    }

    fn build_js_global_fields(&mut self) {
        self.js_global_object_fields = self.common_edge_names(&self.js_global_objects);
        self.js_global_proxy_fields = self.common_edge_names(&self.js_global_proxies);
    }

    fn common_edge_names(&self, ordinals: &[usize]) -> FxHashSet<String> {
        let mut common: Option<FxHashSet<String>> = None;
        for &ord in ordinals {
            let mut fields = FxHashSet::default();
            for (edge_idx, _) in self.iter_edges(NodeOrdinal(ord)) {
                let edge_type = self.edges[edge_idx + self.edge_type_offset];
                if edge_type != self.edge_element_type
                    && edge_type != self.edge_hidden_type
                    && !self.is_invisible_edge(edge_idx)
                {
                    let name_idx = self.edges[edge_idx + self.edge_name_offset] as usize;
                    fields.insert(self.strings[name_idx].clone());
                }
            }
            common = Some(match common {
                None => fields,
                Some(acc) => acc.intersection(&fields).cloned().collect(),
            });
        }
        common.unwrap_or_default()
    }

    fn build_edge_indexes(&mut self) {
        let nfc = self.node_field_count;
        let efc = self.edge_fields_count;
        let eco = self.node_edge_count_offset;
        self.first_edge_indexes[self.node_count] = self.edges.len() as u32;
        let mut edge_index: u32 = 0;
        for ordinal in 0..self.node_count {
            self.first_edge_indexes[ordinal] = edge_index;
            edge_index += self.nodes[ordinal * nfc + eco] * efc as u32;
        }
    }

    fn build_retainers(&mut self) {
        let nfc = self.node_field_count;
        let efc = self.edge_fields_count;
        let eto = self.edge_to_node_offset;

        // edge_to_node_ordinals
        let edge_count = self.edge_count;
        let mut edge_to_node_ordinals = vec![0u32; edge_count];
        for edge_ordinal in 0..edge_count {
            let to_node_index = self.edges[edge_ordinal * efc + eto] as usize;
            edge_to_node_ordinals[edge_ordinal] = (to_node_index / nfc) as u32;
        }

        // Count retainers per node
        let mut first_retainer_index = vec![0u32; self.node_count + 1];
        for &to_ord in &edge_to_node_ordinals {
            first_retainer_index[to_ord as usize] += 1;
        }

        // Prefix sum — also stash each node's retainer count into
        // retaining_nodes[first_unused] so the fill phase can decrement it.
        // Skip the write when count is 0 to avoid an OOB access when
        // first_unused == edge_count (happens with isolated nodes).
        let mut retaining_nodes = vec![0u32; edge_count];
        let mut first_unused = 0u32;
        for i in 0..self.node_count {
            let count = first_retainer_index[i];
            first_retainer_index[i] = first_unused;
            if count > 0 {
                retaining_nodes[first_unused as usize] = count;
            }
            first_unused += count;
        }
        first_retainer_index[self.node_count] = retaining_nodes.len() as u32;

        // Fill retainers
        let mut retaining_edges = vec![0u32; edge_count];
        let mut next_first = self.first_edge_indexes[0];
        for src_ordinal in 0..self.node_count {
            let first_edge = next_first;
            next_first = self.first_edge_indexes[src_ordinal + 1];
            let src_node_index = (src_ordinal * nfc) as u32;
            let mut edge_index = first_edge;
            while edge_index < next_first {
                let to_ordinal = edge_to_node_ordinals[(edge_index / efc as u32) as usize];
                let first_slot = first_retainer_index[to_ordinal as usize] as usize;
                retaining_nodes[first_slot] -= 1;
                let slot = first_slot + retaining_nodes[first_slot] as usize;
                retaining_nodes[slot] = src_node_index;
                retaining_edges[slot] = edge_index;
                edge_index += efc as u32;
            }
        }

        self.retaining_nodes = retaining_nodes;
        self.retaining_edges = retaining_edges;
        self.first_retainer_index = first_retainer_index;
    }

    fn propagate_dom_state(&mut self) {
        if self.node_detachedness_offset == -1 {
            return;
        }

        let nfc = self.node_field_count;
        let det_offset = self.node_detachedness_offset as usize;
        let efc = self.edge_fields_count;
        let eto = self.edge_to_node_offset;
        let etype_off = self.edge_type_offset;

        let mut visited = vec![0u8; self.node_count];
        let mut attached: Vec<NodeOrdinal> = Vec::new();
        let mut detached: Vec<NodeOrdinal> = Vec::new();

        // Read initial detachedness from nodes, identify native nodes
        for ordinal in 0..self.node_count {
            let node_index = ordinal * nfc;
            let node_type = self.nodes[node_index + self.node_type_offset];
            if node_type != self.node_native_type {
                continue;
            }

            let detachedness = self.nodes[node_index + det_offset] & BITMASK_FOR_DOM_LINK_STATE;
            // detachedness: 0 = unknown, 1 = attached, 2 = detached
            if detachedness == 1 {
                attached.push(NodeOrdinal(ordinal));
                visited[ordinal] = 1;
            } else if detachedness == 2 {
                detached.push(NodeOrdinal(ordinal));
                visited[ordinal] = 1;
            }
        }

        // Propagate attached state to all reachable children.
        // NOTE: DevTools only propagates to native (DOM) nodes. We propagate
        // to all node types so the Det column is useful for JS objects too.
        while let Some(ordinal) = attached.pop() {
            let _node_index = ordinal.0 * nfc;
            let first_edge = self.first_edge_indexes[ordinal.0] as usize;
            let last_edge = self.first_edge_indexes[ordinal.0 + 1] as usize;
            let mut edge_index = first_edge;
            while edge_index < last_edge {
                let edge_type = self.edges[edge_index + etype_off];
                if edge_type == self.edge_weak_type || edge_type == self.edge_invisible_type {
                    edge_index += efc;
                    continue;
                }
                let child_index = self.edges[edge_index + eto] as usize;
                let child_ordinal = child_index / nfc;
                if visited[child_ordinal] != 0 {
                    edge_index += efc;
                    continue;
                }
                visited[child_ordinal] = 1;
                // Write attached state back to node data
                let old = self.nodes[child_index + det_offset];
                self.nodes[child_index + det_offset] = (old & !BITMASK_FOR_DOM_LINK_STATE) | 1;
                attached.push(NodeOrdinal(child_ordinal));
                edge_index += efc;
            }
        }

        // Propagate detached state to all reachable children.
        // NOTE: DevTools only propagates to native (DOM) nodes. We propagate
        // to all node types so the Det column is useful for JS objects too.
        while let Some(ordinal) = detached.pop() {
            let _node_index = ordinal.0 * nfc;
            let first_edge = self.first_edge_indexes[ordinal.0] as usize;
            let last_edge = self.first_edge_indexes[ordinal.0 + 1] as usize;
            let mut edge_index = first_edge;
            while edge_index < last_edge {
                let edge_type = self.edges[edge_index + etype_off];
                if edge_type == self.edge_weak_type || edge_type == self.edge_invisible_type {
                    edge_index += efc;
                    continue;
                }
                let child_index = self.edges[edge_index + eto] as usize;
                let child_ordinal = child_index / nfc;
                if visited[child_ordinal] != 0 {
                    edge_index += efc;
                    continue;
                }
                visited[child_ordinal] = 1;
                // Write detached state back to node data
                let old = self.nodes[child_index + det_offset];
                self.nodes[child_index + det_offset] = (old & !BITMASK_FOR_DOM_LINK_STATE) | 2;
                detached.push(NodeOrdinal(child_ordinal));
                edge_index += efc;
            }
        }
    }

    fn calculate_flags(&mut self) {
        self.flags = vec![0u8; self.node_count];
        self.mark_detached_dom_tree_nodes();
        // NOTE: DevTools also calls markQueriableHeapObjects here, which
        // marks nodes reachable from user roots (NativeContexts) through
        // non-hidden/non-weak/non-internal edges as "queryable".  DevTools
        // uses this to filter the heap profiler UI.  We don't use this flag
        // so the pass is omitted.
        //
        // NOTE: DevTools also calls markPageOwnedNodes here, which floods a
        // "pageObject" flag from user roots through non-weak edges.  The
        // flag is used in isEssentialEdge to prevent non-page nodes from
        // dominating page-owned objects (edges from non-page → page-owned
        // are marked non-essential).  Since our target snapshots don't have
        // user roots, the flag is never set and the check has no effect, so
        // the pass is omitted.
    }

    fn mark_detached_dom_tree_nodes(&mut self) {
        if self.node_detachedness_offset == -1 {
            return;
        }
        let nfc = self.node_field_count;
        let det_offset = self.node_detachedness_offset as usize;
        let flag: u8 = 2; // detachedDOMTreeNode
        for ordinal in 0..self.node_count {
            let node_index = ordinal * nfc;
            let node_type = self.nodes[node_index + self.node_type_offset];
            if node_type != self.node_native_type {
                continue;
            }
            if self.nodes[node_index + det_offset] & BITMASK_FOR_DOM_LINK_STATE == 2 {
                self.flags[ordinal] |= flag;
            }
        }
    }

    fn init_essential_edges(&self) -> Vec<bool> {
        let nfc = self.node_field_count;
        let efc = self.edge_fields_count;
        let mut essential = vec![false; self.edge_count];

        // Build weak map edge name regex
        let weak_map_re = Regex::new(
            r"^\d+( / part of key \(.*? @\d+\) -> value \(.*? @\d+\) pair in WeakMap \(table @(\d+)\))$",
        )
        .unwrap();

        for ordinal in 0..self.node_count {
            let node_index = ordinal * nfc;
            let first_edge = self.first_edge_indexes[ordinal] as usize;
            let last_edge = self.first_edge_indexes[ordinal + 1] as usize;
            let mut ei = first_edge;
            while ei < last_edge {
                let edge_ordinal = ei / efc;
                if self.is_essential_edge(node_index, ei, &weak_map_re) {
                    essential[edge_ordinal] = true;
                }
                ei += efc;
            }
        }
        essential
    }

    fn is_essential_edge(&self, node_index: usize, edge_index: usize, weak_map_re: &Regex) -> bool {
        let edge_type = self.edges[edge_index + self.edge_type_offset];

        // Difference from DevTools:
        // WeakMap ephemeron edges are emitted twice: key→value and table→value.
        // DevTools keeps key→value as essential and drops table→value. We do
        // the opposite so the WeakMap table, not the key, dominates the value.
        // That keeps retained sizes from charging ephemeron values to keys that
        // do not actually own them.
        if edge_type == self.edge_internal_type {
            let edge_name_index = self.edges[edge_index + self.edge_name_offset] as usize;
            let edge_name = &self.strings[edge_name_index];
            if let Some(caps) = weak_map_re.captures(edge_name) {
                if let Some(table_id_str) = caps.get(2) {
                    let node_id = self.nodes[node_index + self.node_id_offset];
                    if let Ok(table_id) = table_id_str.as_str().parse::<u32>() {
                        if node_id != table_id {
                            return false;
                        }
                    }
                }
            }
        }

        // Weak edges never retain anything
        if edge_type == self.edge_weak_type {
            return false;
        }

        let child_index = self.edges[edge_index + self.edge_to_node_offset] as usize;
        // Ignore self edges
        if node_index == child_index {
            return false;
        }

        let nfc = self.node_field_count;
        if node_index != self.gc_roots_ordinal * nfc {
            // Similar to DevTools, shortcut edges only have root-entry meaning
            // at the top of the root set, so non-root shortcuts are ignored.
            if edge_type == self.edge_shortcut_type {
                return false;
            }
        }

        // Difference from DevTools:
        // DevTools builds dominators from the synthetic root and therefore
        // keeps synthetic-root entry edges meaningful. We instead treat the
        // synthetic root as a serialization artifact and root dominators at
        // `(GC roots)`, so every outgoing edge from the synthetic root is
        // marked non-essential.
        if node_index == self.root_node_index {
            return false;
        }

        // Difference from DevTools:
        // DevTools also filters non-page→page edges here using the
        // `pageObject` flag populated by `markPageOwnedNodes()`. We omit that
        // branch because our target snapshots do not have user-root/page-owned
        // structure, so the flag would never be meaningfully set.

        true
    }

    fn calculate_dominators_and_retained_sizes(&mut self, essential_edges: &[bool]) {
        let nfc = self.node_field_count;
        let efc = self.edge_fields_count;
        let node_count = self.node_count;
        // DevTools builds the dominator tree from the snapshot's synthetic
        // root and does not mirror the user-root/system-root split it uses for
        // distance BFS. We intentionally root dominators at `(GC roots)`
        // instead, because that node is the logical GC liveness root in our
        // model; anything not reached from it is later attached back to this
        // same root.
        let root_ordinal = self.gc_roots_ordinal;

        // Build edge_to_node_ordinals
        let mut edge_to_node_ordinals = vec![0u32; self.edge_count];
        for edge_ordinal in 0..self.edge_count {
            let to_node_index = self.edges[edge_ordinal * efc + self.edge_to_node_offset] as usize;
            edge_to_node_ordinals[edge_ordinal] = (to_node_index / nfc) as u32;
        }

        // Lengauer-Tarjan algorithm (1-indexed)
        let array_len = node_count + 1;
        let mut parent = vec![0u32; array_len];
        let mut ancestor = vec![0u32; array_len];
        let mut vertex = vec![0u32; array_len];
        let mut label = vec![0u32; array_len];
        let mut semi = vec![0u32; array_len];
        let mut bucket: Vec<Vec<u32>> = vec![Vec::new(); array_len];
        let mut n: u32 = 0;

        // Iterative DFS
        let mut next_edge_index = vec![0u32; array_len];
        {
            let _dfs_stack: Vec<u32> = Vec::new();

            let do_dfs = |root: u32,
                          semi: &mut Vec<u32>,
                          n: &mut u32,
                          vertex: &mut Vec<u32>,
                          label: &mut Vec<u32>,
                          parent: &mut Vec<u32>,
                          next_edge_index: &mut Vec<u32>,
                          first_edge_indexes: &[u32],
                          efc: usize,
                          essential_edges: &[bool],
                          edge_to_node_ordinals: &[u32]| {
                let root_ord = root - 1;
                next_edge_index[root_ord as usize] = first_edge_indexes[root_ord as usize];
                let mut v = root;
                loop {
                    if semi[v as usize] == 0 {
                        *n += 1;
                        semi[v as usize] = *n;
                        vertex[*n as usize] = v;
                        label[v as usize] = v;
                    }
                    let mut v_next = parent[v as usize];
                    let v_ord = (v - 1) as usize;
                    let edge_end = first_edge_indexes[v_ord + 1];
                    while next_edge_index[v_ord] < edge_end {
                        let ei = next_edge_index[v_ord] as usize;
                        let edge_ordinal = ei / efc;
                        if !essential_edges[edge_ordinal] {
                            next_edge_index[v_ord] += efc as u32;
                            continue;
                        }
                        let w_ord = edge_to_node_ordinals[edge_ordinal];
                        let w = w_ord + 1;
                        if semi[w as usize] == 0 {
                            parent[w as usize] = v;
                            next_edge_index[w_ord as usize] = first_edge_indexes[w_ord as usize];
                            next_edge_index[v_ord] += efc as u32;
                            v_next = w;
                            break;
                        }
                        next_edge_index[v_ord] += efc as u32;
                    }
                    if v_next == 0 && v == root {
                        break;
                    }
                    if v_next == parent[v as usize] && v == root {
                        break;
                    }
                    v = v_next;
                    if v == 0 {
                        break;
                    }
                }
            };

            let r = (root_ordinal + 1) as u32;
            do_dfs(
                r,
                &mut semi,
                &mut n,
                &mut vertex,
                &mut label,
                &mut parent,
                &mut next_edge_index,
                &self.first_edge_indexes,
                efc,
                essential_edges,
                &edge_to_node_ordinals,
            );

            // Handle unreachable nodes with only weak retainers
            if (n as usize) < node_count {
                for v in 1..=node_count as u32 {
                    if semi[v as usize] == 0 {
                        let v_ord = (v - 1) as usize;
                        if self.has_only_weak_retainers(NodeOrdinal(v_ord), essential_edges) {
                            parent[v as usize] = r;
                            do_dfs(
                                v,
                                &mut semi,
                                &mut n,
                                &mut vertex,
                                &mut label,
                                &mut parent,
                                &mut next_edge_index,
                                &self.first_edge_indexes,
                                efc,
                                essential_edges,
                                &edge_to_node_ordinals,
                            );
                        }
                    }
                }
            }

            // Handle still unreachable nodes
            if (n as usize) < node_count {
                for v in 1..=node_count as u32 {
                    if semi[v as usize] == 0 {
                        parent[v as usize] = r;
                        n += 1;
                        semi[v as usize] = n;
                        vertex[n as usize] = v;
                        label[v as usize] = v;
                    }
                }
            }
        }

        // Compress and evaluate
        let mut compression_stack = vec![0u32; array_len];

        let compress = |v: u32,
                        ancestor: &mut Vec<u32>,
                        label: &mut Vec<u32>,
                        semi: &Vec<u32>,
                        stack: &mut Vec<u32>| {
            let mut sp = 0usize;
            let mut w = v;
            while ancestor[ancestor[w as usize] as usize] != 0 {
                sp += 1;
                stack[sp] = w;
                w = ancestor[w as usize];
            }
            while sp > 0 {
                let w2 = stack[sp];
                sp -= 1;
                let aw = ancestor[w2 as usize] as usize;
                if semi[label[aw] as usize] < semi[label[w2 as usize] as usize] {
                    label[w2 as usize] = label[aw];
                }
                ancestor[w2 as usize] = ancestor[aw];
            }
        };

        let evaluate = |v: u32,
                        ancestor: &mut Vec<u32>,
                        label: &mut Vec<u32>,
                        semi: &Vec<u32>,
                        stack: &mut Vec<u32>|
         -> u32 {
            if ancestor[v as usize] == 0 {
                return v;
            }
            compress(v, ancestor, label, semi, stack);
            label[v as usize]
        };

        // Main dominator computation
        let r = (root_ordinal + 1) as u32;
        let mut dom = vec![0u32; array_len];

        for i in (2..=n).rev() {
            let w = vertex[i as usize];
            let w_ord = (w - 1) as usize;

            let mut is_orphan = true;
            let begin_ret = self.first_retainer_index[w_ord] as usize;
            let end_ret = self.first_retainer_index[w_ord + 1] as usize;
            for ret_idx in begin_ret..end_ret {
                let ret_edge_index = self.retaining_edges[ret_idx] as usize;
                let ret_edge_ordinal = ret_edge_index / efc;
                if !essential_edges[ret_edge_ordinal] {
                    continue;
                }
                is_orphan = false;
                let v_node_index = self.retaining_nodes[ret_idx] as usize;
                let v_ord = v_node_index / nfc;
                let v = (v_ord + 1) as u32;
                let u = evaluate(v, &mut ancestor, &mut label, &semi, &mut compression_stack);
                if semi[u as usize] < semi[w as usize] {
                    semi[w as usize] = semi[u as usize];
                }
            }
            if is_orphan {
                semi[w as usize] = semi[r as usize];
            }

            let bkt_idx = vertex[semi[w as usize] as usize] as usize;
            bucket[bkt_idx].push(w);
            // Link
            ancestor[w as usize] = parent[w as usize];

            let pw = parent[w as usize] as usize;
            let bkt: Vec<u32> = std::mem::take(&mut bucket[pw]);
            for &v in &bkt {
                let u = evaluate(v, &mut ancestor, &mut label, &semi, &mut compression_stack);
                dom[v as usize] = if semi[u as usize] < semi[v as usize] {
                    u
                } else {
                    parent[w as usize]
                };
            }
        }

        dom[0] = r;
        dom[r as usize] = r;
        for i in 2..=n {
            let w = vertex[i as usize];
            if dom[w as usize] != vertex[semi[w as usize] as usize] {
                dom[w as usize] = dom[dom[w as usize] as usize];
            }
        }

        // Build dominator tree and retained sizes
        let mut dominators_tree = vec![0u32; node_count];
        let mut retained_sizes = vec![0.0f64; node_count];
        for ord in 0..node_count {
            dominators_tree[ord] = dom[ord + 1] - 1;
            retained_sizes[ord] = self.nodes[ord * nfc + self.node_self_size_offset] as f64;
        }

        // Propagate retained sizes up dominator tree
        for i in (2..=n).rev() {
            let ord = (vertex[i as usize] - 1) as usize;
            let dom_ord = dominators_tree[ord] as usize;
            retained_sizes[dom_ord] += retained_sizes[ord];
        }

        self.dominators_tree = dominators_tree;
        self.retained_sizes = retained_sizes;
    }

    fn has_only_weak_retainers(&self, node_ordinal: NodeOrdinal, essential_edges: &[bool]) -> bool {
        let efc = self.edge_fields_count;
        let begin = self.first_retainer_index[node_ordinal.0] as usize;
        let end = self.first_retainer_index[node_ordinal.0 + 1] as usize;
        for ret_idx in begin..end {
            let edge_index = self.retaining_edges[ret_idx] as usize;
            let edge_ordinal = edge_index / efc;
            if essential_edges[edge_ordinal] {
                return false;
            }
        }
        true
    }

    fn build_dominated_nodes(&mut self) {
        let node_count = self.node_count;
        let nfc = self.node_field_count;
        let root_ordinal = self.gc_roots_ordinal;

        let mut index_array = vec![0u32; node_count + 1];

        // Count dominated nodes per dominator (skip the root itself)
        for ordinal in 0..node_count {
            if ordinal == root_ordinal {
                continue;
            }
            let dom = self.dominators_tree[ordinal] as usize;
            index_array[dom] += 1;
        }

        let dominated_count = node_count - 1;
        let mut dominated_nodes = vec![0u32; dominated_count];

        // Prefix sum
        let mut first = 0usize;
        for i in 0..node_count {
            let count = index_array[i] as usize;
            index_array[i] = first as u32;
            if first < dominated_count {
                dominated_nodes[first] = count as u32;
            }
            first += count;
        }
        index_array[node_count] = dominated_nodes.len() as u32;

        // Fill
        for ordinal in 0..node_count {
            if ordinal == root_ordinal {
                continue;
            }
            let dom = self.dominators_tree[ordinal] as usize;
            let dom_ref_idx = index_array[dom] as usize;
            dominated_nodes[dom_ref_idx] -= 1;
            let slot = dom_ref_idx + dominated_nodes[dom_ref_idx] as usize;
            dominated_nodes[slot] = (ordinal * nfc) as u32;
        }

        self.dominated_nodes = dominated_nodes;
        self.first_dominated_node_index = index_array;
    }

    /// Compute BFS distances from `(GC roots)`.
    ///
    /// # How DevTools does it (and why we differ)
    ///
    /// DevTools runs a two-phase BFS starting from the snapshot's synthetic
    /// root (node 0).  The synthetic root has two kinds of children:
    ///
    ///  1. **User roots** — non-synthetic children, typically the
    ///     `NativeContext` objects that represent JS realms.  These are
    ///     seeded at distance 1 in phase 1.
    ///  2. **System roots** — synthetic children like `(GC roots)`,
    ///     `(Internalized strings)`, etc.  These are seeded in phase 2
    ///     at a `BASE_SYSTEM_DISTANCE` offset of 100 000 000.
    ///
    /// Because phase-2 nodes have distances above the display threshold,
    /// DevTools shows their distance as "–".  This means anything only
    /// reachable through `(GC roots)` — the node that actually keeps
    /// everything alive — appears unreachable in the UI.
    ///
    /// # What we do instead
    ///
    /// We BFS from `(GC roots)` directly (distance 0).  Every node that
    /// the GC considers live gets a single meaningful distance regardless
    /// of whether it sits behind a user root or a system sub-root.
    ///
    /// A fallback phase seeds the synthetic root if it was not already
    /// reached, picking up any nodes that are children of the synthetic
    /// root but not reachable from `(GC roots)` (e.g. detached contexts).
    fn calculate_distances(&mut self) {
        let nfc = self.node_field_count;
        let node_count = self.node_count;

        self.node_distances = vec![Distance::NONE; node_count];

        let mut nodes_to_visit = vec![0u32; node_count];
        let mut visit_len: usize;

        let mut pending_ephemerons = rustc_hash::FxHashSet::default();
        let weak_map_re = Regex::new(
            r"^\d+( / part of key \(.*? @\d+\) -> value \(.*? @\d+\) pair in WeakMap \(table @(\d+)\))$"
        ).unwrap();

        let root_ordinal = self.root_node_index / nfc;
        let gc_roots_ordinal = self.gc_roots_ordinal;

        // Phase 1: BFS from (GC roots).  Distance 0 at GC roots, 1 for
        // its direct children (the individual GC sub-roots), and so on.
        self.node_distances[gc_roots_ordinal] = Distance(0);
        nodes_to_visit[0] = (gc_roots_ordinal * nfc) as u32;
        visit_len = 1;
        self.bfs_with_filter(
            &mut nodes_to_visit,
            &mut visit_len,
            &mut pending_ephemerons,
            &weak_map_re,
        );

        // Phase 2: seed system roots that are siblings of (GC roots) under
        // the synthetic root, e.g. "C++ Persistent roots".  User roots
        // (NativeContexts) are intentionally skipped — if they aren't
        // reachable from (GC roots) they are detached and should stay at
        // Distance::NONE.
        self.node_distances[root_ordinal] = Distance(0);
        visit_len = 0;
        let first = self.first_edge_indexes[root_ordinal] as usize;
        let last = self.first_edge_indexes[root_ordinal + 1] as usize;
        let mut ei = first;
        while ei < last {
            let child_index = self.edges[ei + self.edge_to_node_offset] as usize;
            let child_ordinal = child_index / nfc;
            if self.node_distances[child_ordinal] == Distance::NONE
                && self.root_kinds[child_ordinal] != RootKind::UserRoot
            {
                self.node_distances[child_ordinal] = Distance(1);
                nodes_to_visit[visit_len] = child_index as u32;
                visit_len += 1;
            }
            ei += self.edge_fields_count;
        }
        if visit_len > 0 {
            self.bfs_with_filter(
                &mut nodes_to_visit,
                &mut visit_len,
                &mut pending_ephemerons,
                &weak_map_re,
            );
        }
    }

    /// Compute depths within the unreachable subgraph.
    ///
    /// After `calculate_distances()`, every node with `Distance::NONE` is
    /// unreachable from GC roots.  Among those, some are directly referenced
    /// by reachable nodes (e.g. via weak edges that were filtered out during
    /// distance BFS).  We call those "unreachable roots" and assign them
    /// `Distance::UNREACHABLE_BASE`.  We then BFS through only unreachable
    /// nodes, incrementing by 1, so the UI can show them as U+1, U+2, etc.
    ///
    /// Truly isolated unreachable nodes (no path from any reachable node)
    /// also get `Distance::UNREACHABLE_BASE` and display as plain "U".
    fn calculate_unreachable_depths(&mut self) {
        let nfc = self.node_field_count;

        // When weak_is_reachable is set, we first seed nodes that are
        // directly weakly retained by nodes already reachable from the main
        // BFS, giving them min(retainer distance) + 1.  We BFS from those
        // seeds first so that downstream nodes get correct distances before
        // the normal unreachable seeding runs.  This avoids dependence on
        // node serialization order: a node whose weak retainer has a higher
        // ordinal won't be prematurely seeded as U.
        if self.weak_is_reachable {
            let mut weak_seeds: Vec<usize> = Vec::new();
            for ordinal in 0..self.node_count {
                if self.node_distances[ordinal] != Distance::NONE {
                    continue;
                }
                let first = self.first_retainer_index[ordinal] as usize;
                let last = self.first_retainer_index[ordinal + 1] as usize;
                let mut min_weak_reachable_dist = Distance::NONE;
                for idx in first..last {
                    let retainer_ordinal = self.retaining_nodes[idx] as usize / nfc;
                    let ret_dist = self.node_distances[retainer_ordinal];
                    if !ret_dist.is_reachable() || ret_dist >= min_weak_reachable_dist {
                        continue;
                    }
                    let edge_index = self.retaining_edges[idx] as usize;
                    let edge_type = self.edges[edge_index + self.edge_type_offset];
                    if edge_type == self.edge_weak_type {
                        min_weak_reachable_dist = ret_dist;
                    }
                }
                if min_weak_reachable_dist != Distance::NONE {
                    self.node_distances[ordinal] = Distance(min_weak_reachable_dist.0 + 1);
                    weak_seeds.push(ordinal);
                }
            }
            self.unreachable_bfs(&weak_seeds);
        }

        // Phase 1: find unreachable roots.  A node is seeded as U if:
        //  - it has any retainer from a reachable node (the reachable node
        //    points to it via a weak or filtered edge, making it a direct
        //    entry point into the unreachable subgraph), OR
        //  - it has no non-weak retainers at all (orphaned or only
        //    weak-referenced by other unreachable nodes).
        //
        // A node is NOT seeded only if it has a strong retainer from an
        // unreachable node and no retainer from any reachable node — in
        // that case it will get U+N from the BFS.
        let mut seeds: Vec<usize> = Vec::new();
        for ordinal in 0..self.node_count {
            if self.node_distances[ordinal] != Distance::NONE {
                continue;
            }
            let first = self.first_retainer_index[ordinal] as usize;
            let last = self.first_retainer_index[ordinal + 1] as usize;
            let mut has_reachable_retainer = false;
            let mut has_strong_unreachable_retainer = false;
            for idx in first..last {
                let retainer_ordinal = self.retaining_nodes[idx] as usize / nfc;
                let ret_dist = self.node_distances[retainer_ordinal];
                if ret_dist.is_reachable() {
                    has_reachable_retainer = true;
                    break;
                }
                let edge_index = self.retaining_edges[idx] as usize;
                let edge_type = self.edges[edge_index + self.edge_type_offset];
                if edge_type != self.edge_weak_type {
                    has_strong_unreachable_retainer = true;
                }
            }
            if has_reachable_retainer || !has_strong_unreachable_retainer {
                self.node_distances[ordinal] = Distance::UNREACHABLE_BASE;
                seeds.push(ordinal);
            }
        }

        // Phase 2: BFS from the seeds.
        self.unreachable_bfs(&seeds);

        // Phase 3: any remaining NONE nodes form cycles with no root.
        // Pick each one as a new root and BFS from it.
        for ordinal in 0..self.node_count {
            if self.node_distances[ordinal] == Distance::NONE {
                self.node_distances[ordinal] = Distance::UNREACHABLE_BASE;
                self.unreachable_bfs(&[ordinal]);
            }
        }
    }

    /// BFS from seed nodes through non-weak forward edges, visiting only
    /// unreachable nodes still at `Distance::NONE`.
    fn unreachable_bfs(&mut self, seeds: &[usize]) {
        let nfc = self.node_field_count;
        let efc = self.edge_fields_count;
        let eto = self.edge_to_node_offset;
        let etype_off = self.edge_type_offset;

        // Use a min-heap so nodes are always processed in distance order.
        // Without this, seeds at different starting distances cause a plain
        // FIFO queue to visit high-distance seeds before lower-distance
        // children discovered later, permanently overestimating distances
        // where paths from different seeds converge.
        let mut heap: BinaryHeap<Reverse<(u32, usize)>> = BinaryHeap::new();
        for &s in seeds {
            heap.push(Reverse((self.node_distances[s].0, s)));
        }
        while let Some(Reverse((dist, ordinal))) = heap.pop() {
            // A shorter path may have already been recorded; skip stale entries.
            if self.node_distances[ordinal].0 < dist {
                continue;
            }
            let distance = Distance(dist + 1);
            let first_edge = self.first_edge_indexes[ordinal] as usize;
            let last_edge = self.first_edge_indexes[ordinal + 1] as usize;
            let mut ei = first_edge;
            while ei < last_edge {
                let edge_type = self.edges[ei + etype_off];
                if edge_type == self.edge_weak_type && !self.weak_is_reachable {
                    ei += efc;
                    continue;
                }
                if !self.distance_filter_structural(ordinal * nfc, ei) {
                    ei += efc;
                    continue;
                }
                let child_index = self.edges[ei + eto] as usize;
                let child_ordinal = child_index / nfc;
                if self.node_distances[child_ordinal] == Distance::NONE
                    || distance < self.node_distances[child_ordinal]
                {
                    self.node_distances[child_ordinal] = distance;
                    heap.push(Reverse((distance.0, child_ordinal)));
                }
                ei += efc;
            }
        }
    }

    /// Find the `(GC roots)` node among the root's direct children.
    fn find_gc_roots_ordinal(&self) -> Option<usize> {
        let synthetic_root = NodeOrdinal(self.root_node_index / self.node_field_count);
        self.find_child_by_node_name(synthetic_root, "(GC roots)")
            .map(|o| o.0)
    }

    fn bfs_with_filter(
        &mut self,
        nodes_to_visit: &mut [u32],
        visit_len: &mut usize,
        pending_ephemerons: &mut rustc_hash::FxHashSet<String>,
        weak_map_re: &Regex,
    ) {
        let nfc = self.node_field_count;
        let efc = self.edge_fields_count;
        let eto = self.edge_to_node_offset;
        let etype_off = self.edge_type_offset;

        let mut index = 0;
        while index < *visit_len {
            let node_index = nodes_to_visit[index] as usize;
            index += 1;
            let node_ordinal = node_index / nfc;
            let distance = Distance(self.node_distances[node_ordinal].0 + 1);
            let first_edge = self.first_edge_indexes[node_ordinal] as usize;
            let last_edge = self.first_edge_indexes[node_ordinal + 1] as usize;
            let mut ei = first_edge;
            while ei < last_edge {
                let edge_type = self.edges[ei + etype_off];
                if edge_type == self.edge_weak_type {
                    ei += efc;
                    continue;
                }
                let child_index = self.edges[ei + eto] as usize;
                let child_ordinal = child_index / nfc;
                if self.node_distances[child_ordinal] != Distance::NONE {
                    ei += efc;
                    continue;
                }

                if !self.distance_filter_stateful(node_index, ei, pending_ephemerons, weak_map_re) {
                    ei += efc;
                    continue;
                }

                self.node_distances[child_ordinal] = distance;
                nodes_to_visit[*visit_len] = child_index as u32;
                *visit_len += 1;
                ei += efc;
            }
        }
    }

    fn distance_filter_stateful(
        &self,
        node_index: usize,
        edge_index: usize,
        pending_ephemerons: &mut rustc_hash::FxHashSet<String>,
        weak_map_re: &Regex,
    ) -> bool {
        if !self.distance_filter_structural(node_index, edge_index) {
            return false;
        }

        let edge_name_or_index = self.edges[edge_index + self.edge_name_offset];
        let edge_type = self.edges[edge_index + self.edge_type_offset];

        // WeakMap ephemeron filtering
        if edge_type == self.edge_internal_type {
            let edge_name_index = edge_name_or_index as usize;
            if edge_name_index < self.strings.len() {
                let edge_name = &self.strings[edge_name_index];
                if let Some(caps) = weak_map_re.captures(edge_name) {
                    if let Some(dup) = caps.get(1) {
                        let dup_part = dup.as_str().to_string();
                        if !pending_ephemerons.remove(&dup_part) {
                            pending_ephemerons.insert(dup_part);
                            return false;
                        }
                    }
                }
            }
        }

        true
    }

    /// Stateless structural edge filter shared by the main BFS and
    /// `unreachable_bfs`.  Returns `false` for edges that should never
    /// contribute to distance (sloppy_function_map, descriptor-array
    /// internals).  Does *not* cover the stateful WeakMap/ephemeron
    /// filter which is only relevant during the initial reachable BFS.
    fn distance_filter_structural(&self, node_index: usize, edge_index: usize) -> bool {
        let nfc = self.node_field_count;
        let edge_type = self.edges[edge_index + self.edge_type_offset];
        let edge_name_or_index = self.edges[edge_index + self.edge_name_offset];

        // Filter sloppy_function_map in NativeContext
        if self.is_native_context(NodeOrdinal(node_index / nfc)) {
            if edge_type != self.edge_element_type && edge_type != self.edge_hidden_type {
                let edge_name = &self.strings[edge_name_or_index as usize];
                if edge_name == "sloppy_function_map" {
                    return false;
                }
            }
        }

        // Filter descriptor array edges
        let node_type = self.nodes[node_index + self.node_type_offset];
        if node_type == self.node_array_type {
            let node_name = &self.strings[self.nodes[node_index + self.node_name_offset] as usize];
            if node_name == "(map descriptors)" {
                if edge_type == self.edge_element_type || edge_type == self.edge_hidden_type {
                    let index = edge_name_or_index;
                    if !(index < 2 || (index % 3) != 1) {
                        return false;
                    }
                }
            }
        }

        true
    }

    fn calculate_object_names(&mut self) {
        let nfc = self.node_field_count;
        let node_count = self.node_count;

        if self.node_detachedness_offset == -1 {
            self.detachedness_and_class_index = vec![0u32; node_count];
            self.use_separate_class_index = true;
        }

        let mut string_table: FxHashMap<String, u32> = FxHashMap::default();

        let get_index =
            |s: &str, strings: &mut Vec<String>, table: &mut FxHashMap<String, u32>| -> u32 {
                if let Some(&idx) = table.get(s) {
                    idx
                } else {
                    let idx = strings.len() as u32;
                    strings.push(s.to_string());
                    table.insert(s.to_string(), idx);
                    idx
                }
            };

        let hidden_idx = get_index("(system)", &mut self.strings, &mut string_table);
        let code_idx = get_index("(compiled code)", &mut self.strings, &mut string_table);
        let function_idx = get_index("Function", &mut self.strings, &mut string_table);
        let regexp_idx = get_index("RegExp", &mut self.strings, &mut string_table);

        for ordinal in 0..node_count {
            let node_index = ordinal * nfc;
            let raw_type = self.nodes[node_index + self.node_type_offset];
            let raw_name_idx = self.nodes[node_index + self.node_name_offset];

            let class_index = if raw_type == self.node_hidden_type {
                hidden_idx
            } else if raw_type == self.node_object_type || raw_type == self.node_native_type {
                let name = self.strings[raw_name_idx as usize].clone();
                if let Some(normalized) = Self::normalize_constructor_type(&name) {
                    get_index(normalized, &mut self.strings, &mut string_table)
                } else if name.starts_with('<') {
                    let first_space = name.find(' ');
                    let short_name = if let Some(pos) = first_space {
                        format!("{}>", &name[..pos])
                    } else {
                        name
                    };
                    get_index(&short_name, &mut self.strings, &mut string_table)
                } else {
                    // Use raw name index directly
                    raw_name_idx
                }
            } else if raw_type == self.node_code_type {
                code_idx
            } else if raw_type == self.node_closure_type {
                function_idx
            } else if raw_type == self.node_regexp_type {
                regexp_idx
            } else {
                // Other types: "(type_name)"
                let type_name = if (raw_type as usize) < self.node_types.len() {
                    self.node_types[raw_type as usize].clone()
                } else {
                    format!("unknown_{}", raw_type)
                };
                get_index(
                    &format!("({})", type_name),
                    &mut self.strings,
                    &mut string_table,
                )
            };

            self.set_class_index(NodeOrdinal(ordinal), class_index);
        }
    }

    fn normalize_constructor_type(name: &str) -> Option<&'static str> {
        if Self::has_global_suffix(name, " (global*)") {
            Some("[JSGlobalObject]")
        } else if Self::has_global_suffix(name, " (global)") {
            Some("[JSGlobalProxy]")
        } else {
            None
        }
    }

    fn has_global_suffix(name: &str, suffix: &str) -> bool {
        name.strip_suffix(suffix).is_some() || name.contains(&format!("{suffix} / "))
    }

    fn normalize_display_name(name: &str) -> String {
        if name.contains(" (global*)") {
            name.replacen(" (global*)", " [JSGlobalObject]", 1)
        } else if name.contains(" (global)") {
            name.replacen(" (global)", " [JSGlobalProxy]", 1)
        } else {
            name.to_string()
        }
    }

    fn infer_and_apply_interface_definitions(&mut self) {
        let efc = self.edge_fields_count;
        let nfc = self.node_field_count;
        let edge_prop_type = self.edge_property_type;

        // Phase 1: Collect interface candidates
        struct Candidate {
            name: String,
            properties: Vec<String>,
            count: u32,
        }

        let mut candidates: FxHashMap<String, Candidate> = FxHashMap::default();
        let mut total_object_count = 0u32;

        for ordinal in 0..self.node_count {
            let node_index = ordinal * nfc;
            let raw_type = self.nodes[node_index + self.node_type_offset];
            let raw_name_idx = self.nodes[node_index + self.node_name_offset] as usize;
            if raw_type != self.node_object_type || self.strings[raw_name_idx] != "Object" {
                continue;
            }
            total_object_count += 1;

            let mut interface_name = "{".to_string();
            let mut properties: Vec<String> = Vec::new();
            let first_edge = self.first_edge_indexes[ordinal] as usize;
            let last_edge = self.first_edge_indexes[ordinal + 1] as usize;
            let mut ei = first_edge;
            while ei < last_edge {
                let et = self.edges[ei + self.edge_type_offset];
                if et != edge_prop_type {
                    ei += efc;
                    continue;
                }
                let name_idx = self.edges[ei + self.edge_name_offset] as usize;
                let edge_name = self.strings[name_idx].clone();
                if edge_name == "__proto__" {
                    ei += efc;
                    continue;
                }
                let formatted = Self::format_property_name(&edge_name);
                if interface_name.len() > MIN_INTERFACE_PROPERTY_COUNT
                    && interface_name.len() + formatted.len() > MAX_INTERFACE_NAME_LENGTH
                {
                    break;
                }
                if interface_name.len() != 1 {
                    interface_name.push_str(", ");
                }
                interface_name.push_str(&formatted);
                properties.push(edge_name);
                ei += efc;
            }
            interface_name.push('}');

            if properties.is_empty() {
                continue;
            }

            candidates
                .entry(interface_name.clone())
                .and_modify(|c| c.count += 1)
                .or_insert(Candidate {
                    name: interface_name,
                    properties,
                    count: 1,
                });
        }

        // Phase 2: Sort by count descending, filter by min count
        let min_count = 2u32.max(total_object_count / 1000);
        let mut sorted_candidates: Vec<_> = candidates.into_values().collect();
        sorted_candidates.sort_by(|a, b| b.count.cmp(&a.count));

        let definitions: Vec<_> = sorted_candidates
            .into_iter()
            .take_while(|c| c.count >= min_count)
            .collect();

        if definitions.is_empty() {
            return;
        }

        // Phase 3: Build property trie from sorted properties
        struct TrieNode {
            next: FxHashMap<String, usize>, // property -> trie node index
            match_name: Option<(String, usize, usize)>, // (name, property_count, definition_index)
            greatest_next: Option<String>,
        }

        let mut trie_nodes: Vec<TrieNode> = vec![TrieNode {
            next: FxHashMap::default(),
            match_name: None,
            greatest_next: None,
        }];

        for (def_idx, def) in definitions.iter().enumerate() {
            let mut sorted_props = def.properties.clone();
            sorted_props.sort();

            let mut current = 0usize; // trie root
            for prop in &sorted_props {
                let next_idx = if let Some(&idx) = trie_nodes[current].next.get(prop) {
                    idx
                } else {
                    let idx = trie_nodes.len();
                    trie_nodes.push(TrieNode {
                        next: FxHashMap::default(),
                        match_name: None,
                        greatest_next: None,
                    });
                    trie_nodes[current].next.insert(prop.clone(), idx);
                    let should_update = trie_nodes[current]
                        .greatest_next
                        .as_ref()
                        .is_none_or(|g| g < prop);
                    if should_update {
                        trie_nodes[current].greatest_next = Some(prop.clone());
                    }
                    idx
                };
                current = next_idx;
            }
            // Only set match if not already set (earlier definitions have priority)
            if trie_nodes[current].match_name.is_none() {
                trie_nodes[current].match_name =
                    Some((def.name.clone(), sorted_props.len(), def_idx));
            }
        }

        // Phase 4: Apply definitions to all plain Objects
        let mut interface_names: FxHashMap<String, u32> = FxHashMap::default();

        for ordinal in 0..self.node_count {
            let node_index = ordinal * nfc;
            let raw_type = self.nodes[node_index + self.node_type_offset];
            let raw_name_idx = self.nodes[node_index + self.node_name_offset] as usize;
            if raw_type != self.node_object_type || self.strings[raw_name_idx] != "Object" {
                continue;
            }

            // Collect and sort properties
            let mut properties: Vec<String> = Vec::new();
            let first_edge = self.first_edge_indexes[ordinal] as usize;
            let last_edge = self.first_edge_indexes[ordinal + 1] as usize;
            let mut ei = first_edge;
            while ei < last_edge {
                let et = self.edges[ei + self.edge_type_offset];
                if et == edge_prop_type {
                    let name_idx = self.edges[ei + self.edge_name_offset] as usize;
                    properties.push(self.strings[name_idx].clone());
                }
                ei += efc;
            }
            properties.sort();

            // Traverse trie to find best match
            let mut states: Vec<usize> = vec![0]; // start at root
            // Best match: (name, property_count, definition_index)
            let mut best: Option<(String, usize, usize)> = trie_nodes[0].match_name.clone();

            for prop in &properties {
                let current_states: Vec<usize> = states.clone();
                for &state in &current_states {
                    // Remove state if no further transitions possible
                    if let Some(ref greatest) = trie_nodes[state].greatest_next {
                        if prop >= greatest {
                            states.retain(|&s| s != state);
                        }
                    } else {
                        states.retain(|&s| s != state);
                    }

                    // Try to transition
                    if let Some(&next_state) = trie_nodes[state].next.get(prop) {
                        if !states.contains(&next_state) {
                            states.push(next_state);
                        }
                        if let Some(ref m) = trie_nodes[next_state].match_name {
                            let is_better = match &best {
                                None => true,
                                Some(b) => {
                                    if m.1 > b.1 {
                                        true
                                    } else if m.1 < b.1 {
                                        false
                                    } else {
                                        m.2 <= b.2
                                    }
                                }
                            };
                            if is_better {
                                best = Some(m.clone());
                            }
                        }
                    }
                }
            }

            // Apply match
            if let Some((ref match_name, _, _)) = best {
                let class_idx = if let Some(&idx) = interface_names.get(match_name) {
                    idx
                } else {
                    let idx = self.strings.len() as u32;
                    self.strings.push(match_name.clone());
                    interface_names.insert(match_name.clone(), idx);
                    idx
                };
                self.set_class_index(NodeOrdinal(ordinal), class_idx);
            }
        }
    }

    fn format_property_name(name: &str) -> String {
        // Accessors show as "get name" or "set name"
        if name.starts_with("get ") || name.starts_with("set ") {
            return name.to_string();
        }
        // Symbols show as "<symbol name>"
        if name.starts_with("Symbol(") {
            return format!("<symbol {}>", &name[7..name.len().saturating_sub(1)]);
        }
        name.to_string()
    }

    fn set_class_index(&mut self, ordinal: NodeOrdinal, index: u32) {
        if self.use_separate_class_index {
            self.detachedness_and_class_index[ordinal.0] = index << SHIFT_FOR_CLASS_INDEX;
        } else {
            let det_off = self.node_detachedness_offset as usize;
            let node_index = ordinal.0 * self.node_field_count;
            let mut val = self.nodes[node_index + det_off];
            val &= BITMASK_FOR_DOM_LINK_STATE;
            val |= index << SHIFT_FOR_CLASS_INDEX;
            self.nodes[node_index + det_off] = val;
        }
    }

    fn class_index(&self, ordinal: NodeOrdinal) -> u32 {
        if self.use_separate_class_index {
            self.detachedness_and_class_index[ordinal.0] >> SHIFT_FOR_CLASS_INDEX
        } else {
            let det_off = self.node_detachedness_offset as usize;
            let node_index = ordinal.0 * self.node_field_count;
            self.nodes[node_index + det_off] >> SHIFT_FOR_CLASS_INDEX
        }
    }

    fn class_key_internal(&self, ordinal: NodeOrdinal) -> ClassKey {
        let node_index = ordinal.0 * self.node_field_count;
        let raw_type = self.nodes[node_index + self.node_type_offset];
        if raw_type != self.node_object_type {
            return ClassKey::Index(self.class_index(ordinal));
        }
        if let Some(&loc) = self.location_map.get(&node_index) {
            ClassKey::Location(
                loc.script_id,
                loc.line,
                loc.column,
                self.node_class_name(ordinal),
            )
        } else {
            ClassKey::Index(self.class_index(ordinal))
        }
    }

    fn calculate_statistics(&mut self) {
        let nfc = self.node_field_count;
        let sso = self.node_self_size_offset;
        let tyo = self.node_type_offset;
        let nmo = self.node_name_offset;
        let _efc = self.edge_fields_count;

        let mut size_native = self.extra_native_bytes;
        let mut size_typed_arrays = 0.0f64;
        let mut size_code = 0.0f64;
        let mut size_strings = 0.0f64;
        let mut size_js_arrays = 0.0f64;
        let mut size_system = 0.0f64;
        let mut unreachable_count = 0u32;
        let mut unreachable_size = 0.0f64;

        for ordinal in 0..self.node_count {
            let node_index = ordinal * nfc;
            let node_size = self.nodes[node_index + sso] as f64;
            let node_type = self.nodes[node_index + tyo];

            if self.node_distances[ordinal].is_unreachable() && node_size > 0.0 {
                unreachable_count += 1;
                unreachable_size += node_size;
            }

            if node_type == self.node_hidden_type {
                size_system += node_size;
                continue;
            }

            if node_type == self.node_native_type {
                size_native += node_size;
                let name = &self.strings[self.nodes[node_index + nmo] as usize];
                if name == "system / JSArrayBufferData" {
                    size_typed_arrays += node_size;
                }
            } else if node_type == self.node_code_type {
                size_code += node_size;
            } else if node_type == self.node_cons_string_type
                || node_type == self.node_sliced_string_type
                || node_type == self.node_string_type
            {
                size_strings += node_size;
            } else {
                let name = &self.strings[self.nodes[node_index + nmo] as usize];
                if name == "Array" {
                    size_js_arrays += self.calculate_array_size(NodeOrdinal(ordinal));
                }
            }
        }

        let total = self.retained_sizes[self.gc_roots_ordinal] + self.extra_native_bytes;

        self.statistics = Statistics {
            total,
            native_total: size_native,
            typed_arrays: size_typed_arrays,
            v8heap_total: total - size_native,
            code: size_code,
            js_arrays: size_js_arrays,
            strings: size_strings,
            system: size_system,
            extra_native_bytes: self.extra_native_bytes,
            unreachable_count,
            unreachable_size,
        };
    }

    fn calculate_array_size(&self, ordinal: NodeOrdinal) -> f64 {
        let nfc = self.node_field_count;
        let node_index = ordinal.0 * nfc;
        let mut size = self.nodes[node_index + self.node_self_size_offset] as f64;

        let first_edge = self.first_edge_indexes[ordinal.0] as usize;
        let last_edge = self.first_edge_indexes[ordinal.0 + 1] as usize;
        let efc = self.edge_fields_count;
        let mut ei = first_edge;
        while ei < last_edge {
            let et = self.edges[ei + self.edge_type_offset];
            if et != self.edge_internal_type {
                ei += efc;
                continue;
            }
            // Check if edge name is "elements"
            let name_idx = self.edges[ei + self.edge_name_offset] as usize;
            if self.strings[name_idx] != "elements" {
                ei += efc;
                continue;
            }
            let elements_node_index = self.edges[ei + self.edge_to_node_offset] as usize;
            let elements_ordinal = elements_node_index / nfc;
            // Check retainers count
            let ret_count = self.first_retainer_index[elements_ordinal + 1]
                - self.first_retainer_index[elements_ordinal];
            if ret_count == 1 {
                size += self.nodes[elements_node_index + self.node_self_size_offset] as f64;
            }
            break;
        }
        size
    }

    // Public API

    pub fn native_contexts(&self) -> &[NativeContextData] {
        &self.native_contexts
    }

    pub fn native_context_by_id(&self, id: NativeContextId) -> &NativeContextData {
        &self.native_contexts[id.0 as usize]
    }

    pub fn native_context_id(&self, ordinal: NodeOrdinal) -> Option<NativeContextId> {
        self.native_contexts
            .iter()
            .position(|ctx| ctx.ordinal == ordinal)
            .map(|idx| NativeContextId(idx as u32))
    }

    pub fn native_context_data(&self, ordinal: NodeOrdinal) -> Option<&NativeContextData> {
        self.native_contexts
            .iter()
            .find(|ctx| ctx.ordinal == ordinal)
    }

    pub fn native_context_attributable_sizes(&self) -> NativeContextAttributableSizes {
        NativeContextAttributableSizes {
            native_contexts: self.native_contexts.clone(),
            shared: self.shared_attributable_size,
            unattributed: self.unattributed_size,
        }
    }

    pub fn native_context_attributable_size(&self, ordinal: NodeOrdinal) -> Option<f64> {
        self.native_context_id(ordinal)
            .map(|id| self.native_context_by_id(id).size)
    }

    pub fn shared_attributable_size(&self) -> f64 {
        self.shared_attributable_size
    }

    pub fn unattributed_size(&self) -> f64 {
        self.unattributed_size
    }

    /// Returns sorted edge names of this NativeContext's global_object that are
    /// not common to all NativeContext global_objects.
    /// Returns the precomputed vars string for a NativeContext.
    pub fn native_context_vars(&self, ord: NodeOrdinal) -> &str {
        self.native_context_vars
            .get(&ord)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    fn native_context_global_unique_fields(&self, ord: NodeOrdinal) -> Vec<String> {
        let Some(global) = self.find_edge_target(ord, "global_object") else {
            return Vec::new();
        };
        let mut unique: Vec<String> = self
            .iter_edges(global)
            .filter_map(|(edge_idx, _)| {
                let edge_type = self.edges[edge_idx + self.edge_type_offset];
                if edge_type == self.edge_element_type || edge_type == self.edge_hidden_type {
                    return None;
                }
                let name_idx = self.edges[edge_idx + self.edge_name_offset] as usize;
                let name = &self.strings[name_idx];
                if self.native_context_global_fields.contains(name.as_str()) {
                    None
                } else {
                    Some(name.clone())
                }
            })
            .collect();
        unique.sort();
        unique
    }

    /// Returns sorted variable names from the script_context_table of a NativeContext.
    /// These are `let`/`const` declarations at the top-level script scope.
    fn native_context_script_context_vars(&self, ord: NodeOrdinal) -> Vec<String> {
        let Some(table) = self.find_edge_target(ord, "script_context_table") else {
            return Vec::new();
        };
        let mut vars = Vec::new();
        // The ScriptContextTable has hidden edges to Context objects.
        for (edge_idx, child_ord) in self.iter_edges(table) {
            let edge_type = self.edges[edge_idx + self.edge_type_offset];
            if edge_type != self.edge_hidden_type && edge_type != self.edge_element_type {
                continue;
            }
            // Each Context has "context"-typed edges for its variables.
            for (ctx_edge_idx, _) in self.iter_edges(child_ord) {
                let ctx_edge_type = self.edges[ctx_edge_idx + self.edge_type_offset];
                if ctx_edge_type != self.edge_context_type {
                    continue;
                }
                let name_idx = self.edges[ctx_edge_idx + self.edge_name_offset] as usize;
                let name = &self.strings[name_idx];
                // Skip "this" and other internal context vars.
                if name != "this" {
                    vars.push(name.clone());
                }
            }
        }
        vars.sort();
        vars.dedup();
        vars
    }

    #[cfg(test)]
    pub fn js_global_objects(&self) -> &[usize] {
        &self.js_global_objects
    }

    #[cfg(test)]
    pub fn js_global_proxies(&self) -> &[usize] {
        &self.js_global_proxies
    }

    pub fn is_js_global_object(&self, ordinal: NodeOrdinal) -> bool {
        matches!(
            Self::normalize_constructor_type(self.node_raw_name(ordinal)),
            Some("[JSGlobalObject]")
        )
    }

    pub fn is_js_global_proxy(&self, ordinal: NodeOrdinal) -> bool {
        matches!(
            Self::normalize_constructor_type(self.node_raw_name(ordinal)),
            Some("[JSGlobalProxy]")
        )
    }

    pub fn is_common_js_global_field(&self, ordinal: NodeOrdinal, name: &str) -> bool {
        if self.is_js_global_object(ordinal) {
            self.js_global_object_fields.contains(name)
        } else if self.is_js_global_proxy(ordinal) {
            self.js_global_proxy_fields.contains(name)
        } else {
            false
        }
    }

    /// The `(GC roots)` node — logical root of the dominator tree and
    /// retained-size computation.
    #[allow(dead_code)]
    pub fn gc_roots_ordinal(&self) -> NodeOrdinal {
        NodeOrdinal(self.gc_roots_ordinal)
    }

    /// The snapshot's synthetic root (node 0).  Use this for views like
    /// containment that want to show `(GC roots)` as a visible child.
    pub fn synthetic_root_ordinal(&self) -> NodeOrdinal {
        NodeOrdinal(self.root_node_index / self.node_field_count)
    }

    /// Returns true when `ordinal` is a user root — a non-synthetic direct
    /// child of the synthetic root (typically a NativeContext).
    pub fn is_user_root(&self, ordinal: NodeOrdinal) -> bool {
        self.root_kinds[ordinal.0] == RootKind::UserRoot
    }

    pub fn root_kind(&self, ordinal: NodeOrdinal) -> RootKind {
        self.root_kinds[ordinal.0]
    }

    #[allow(dead_code)]
    pub fn node_type_name(&self, ordinal: NodeOrdinal) -> &str {
        let t = self.nodes[ordinal.0 * self.node_field_count + self.node_type_offset] as usize;
        if t < self.node_types.len() {
            &self.node_types[t]
        } else {
            "unknown"
        }
    }

    /// Build the script_id -> script name map by scanning nodes with locations
    /// and following edges to Script nodes.
    fn compute_script_names(&mut self) {
        let mut needed: FxHashSet<u32> = self
            .location_map
            .values()
            .map(|loc| loc.script_id)
            .collect();
        for (&node_index, loc) in &self.location_map {
            if !needed.contains(&loc.script_id) {
                continue;
            }
            let ordinal = NodeOrdinal(node_index / self.node_field_count);
            if let Some(name) = self.find_script_name(ordinal) {
                self.script_names.insert(loc.script_id, name);
                needed.remove(&loc.script_id);
                if needed.is_empty() {
                    break;
                }
            }
        }
    }

    /// Follow edges from `ordinal` to find a Script node and return its name.
    /// Handles both SFI (direct "script" edge) and JSFunction ("shared" -> SFI -> "script").
    fn find_script_name(&self, ordinal: NodeOrdinal) -> Option<String> {
        if self.is_shared_function_info(ordinal) {
            for (edge_idx, child_ord) in self.iter_edges(ordinal) {
                if self.edge_name(edge_idx) == "script" {
                    let raw = self.node_raw_name(child_ord);
                    if let Some(name) = raw.strip_prefix("system / Script / ") {
                        return Some(name.to_string());
                    }
                    return None;
                }
            }
        } else if self.is_js_function(ordinal) {
            for (edge_idx, child_ord) in self.iter_edges(ordinal) {
                if self.edge_name(edge_idx) == "shared" {
                    return self.find_script_name(child_ord);
                }
            }
        }
        None
    }

    /// Returns true if this node is a JSFunction (closure type).
    pub fn is_js_function(&self, ordinal: NodeOrdinal) -> bool {
        let node_index = ordinal.0 * self.node_field_count;
        self.nodes[node_index + self.node_type_offset] == self.node_closure_type
    }

    /// Returns true if this node is a SharedFunctionInfo.
    /// SFI nodes have V8 node type "code".
    pub fn is_shared_function_info(&self, ordinal: NodeOrdinal) -> bool {
        let node_index = ordinal.0 * self.node_field_count;
        if self.nodes[node_index + self.node_type_offset] != self.node_code_type {
            return false;
        }
        let name = self.node_raw_name(ordinal);
        name == "system / SharedFunctionInfo" || name.starts_with("system / SharedFunctionInfo / ")
    }

    /// Returns source location data for a node, if available.
    ///
    /// For JSFunction nodes, if the node itself has no location,
    /// follows the "shared" edge to the SharedFunctionInfo and returns its location.
    pub fn node_location(&self, ordinal: NodeOrdinal) -> Option<SourceLocation> {
        let node_index = ordinal.0 * self.node_field_count;
        if let Some(&loc) = self.location_map.get(&node_index) {
            return Some(loc);
        }
        if self.is_js_function(ordinal) {
            for (edge_idx, child_ord) in self.iter_edges(ordinal) {
                if self.edge_name(edge_idx) == "shared" {
                    let child_index = child_ord.0 * self.node_field_count;
                    if let Some(&loc) = self.location_map.get(&child_index) {
                        return Some(loc);
                    }
                    break;
                }
            }
        }
        None
    }

    /// Formats a source location as `"file.js:2:17"` or `"script_id=3:2:17"` if
    /// the script name could not be resolved.
    pub fn format_location(&self, loc: &SourceLocation) -> String {
        let source = match self.script_names.get(&loc.script_id) {
            Some(name) => name.as_str(),
            None => {
                return format!(
                    "script_id={}:L{}:{}",
                    loc.script_id,
                    loc.line + 1,
                    loc.column + 1
                );
            }
        };
        source
            .rsplit_once('/')
            .map_or(source, |(_, file)| file)
            .to_string()
            + &format!(":{}:{}", loc.line + 1, loc.column + 1)
    }

    // --- Allocation trace data ---

    fn build_trace_functions(
        &mut self,
        trace_function_infos: &[u32],
        meta: &crate::types::SnapshotMeta,
    ) {
        let fields = &meta.trace_function_info_fields;
        let fcount = fields.len().max(6);
        let name_off = fields.iter().position(|f| f == "name").unwrap_or(1);
        let script_name_off = fields.iter().position(|f| f == "script_name").unwrap_or(2);
        let line_off = fields.iter().position(|f| f == "line").unwrap_or(4);
        let column_off = fields.iter().position(|f| f == "column").unwrap_or(5);

        let count = trace_function_infos.len() / fcount;
        let mut functions = Vec::with_capacity(count);
        for i in 0..count {
            let base = i * fcount;
            let name_idx = trace_function_infos[base + name_off] as usize;
            let script_idx = trace_function_infos[base + script_name_off] as usize;
            functions.push(AllocationFrame {
                function_name: self.strings.get(name_idx).cloned().unwrap_or_default(),
                script_name: self.strings.get(script_idx).cloned().unwrap_or_default(),
                line: trace_function_infos[base + line_off],
                column: trace_function_infos[base + column_off],
            });
        }
        self.trace_functions = functions;
    }

    /// Returns true if the snapshot contains allocation tracking data.
    pub fn has_allocation_data(&self) -> bool {
        self.node_trace_node_id_offset >= 0 && !self.trace_parents.is_empty()
    }

    /// Get the allocation call stack for a node, innermost frame first.
    pub fn get_allocation_stack(&self, ordinal: NodeOrdinal) -> Option<Vec<AllocationFrame>> {
        if !self.has_allocation_data() {
            return None;
        }
        let node_index = ordinal.0 * self.node_field_count;
        let trace_id = self.nodes[node_index + self.node_trace_node_id_offset as usize] as usize;
        if trace_id == 0 || trace_id >= self.trace_parents.len() {
            return None;
        }

        let mut stack = Vec::new();
        let mut current = trace_id;
        loop {
            let parent = self.trace_parents[current] as usize;
            // Skip the root trace node (parent == 0) — it's V8's synthetic "(root)"
            if parent == 0 || parent == current || parent >= self.trace_parents.len() {
                break;
            }
            let fi_idx = self.trace_func_idxs[current] as usize;
            if fi_idx < self.trace_functions.len() {
                let frame = &self.trace_functions[fi_idx];
                if !frame.function_name.is_empty() {
                    stack.push(frame.clone());
                }
            }
            current = parent;
        }
        if stack.is_empty() { None } else { Some(stack) }
    }

    /// Format an allocation frame as "function (script:line:col)".
    pub fn format_allocation_frame(frame: &AllocationFrame) -> String {
        let script = if frame.script_name.is_empty() {
            "<unknown>".to_string()
        } else {
            frame
                .script_name
                .rsplit_once('/')
                .map_or(frame.script_name.as_str(), |(_, file)| file)
                .to_string()
        };
        format!(
            "{} ({}:{}:{})",
            frame.function_name,
            script,
            frame.line + 1,
            frame.column + 1,
        )
    }

    // --- Allocation timeline ---

    fn build_timeline(&mut self, samples: &[u32], meta: &crate::types::SnapshotMeta) {
        let fields = &meta.sample_fields;
        let fcount = fields.len().max(2);
        let ts_off = fields.iter().position(|f| f == "timestamp_us").unwrap_or(0);
        let id_off = fields
            .iter()
            .position(|f| f == "last_assigned_id")
            .unwrap_or(1);

        // Parse sample entries: [(timestamp_us, last_assigned_id), ...]
        let mut sample_entries: Vec<(u64, u64)> = Vec::new();
        let mut i = 0;
        while i + fcount <= samples.len() {
            let ts = samples[i + ts_off] as u64;
            let last_id = samples[i + id_off] as u64;
            sample_entries.push((ts, last_id));
            i += fcount;
        }

        if sample_entries.is_empty() {
            return;
        }

        // Collect all live object IDs and their sizes, sorted by ID.
        let nfc = self.node_field_count;
        let ido = self.node_id_offset;
        let sso = self.node_self_size_offset;
        let mut objects: Vec<(u64, u32)> = Vec::with_capacity(self.node_count);
        for ordinal in 0..self.node_count {
            let ni = ordinal * nfc;
            let id = self.nodes[ni + ido] as u64;
            let size = self.nodes[ni + sso];
            if size > 0 {
                objects.push((id, size));
            }
        }
        objects.sort_unstable_by_key(|&(id, _)| id);

        // For each interval between consecutive samples, count live objects
        // whose ID falls in [prev_last_id+1, this_last_id].
        let mut intervals = Vec::with_capacity(sample_entries.len());
        let mut obj_idx = 0;
        let mut prev_last_id = 0u64;

        for &(ts, last_id) in &sample_entries {
            // Skip objects below this interval
            while obj_idx < objects.len() && objects[obj_idx].0 <= prev_last_id {
                obj_idx += 1;
            }
            // Count objects in [prev_last_id+1, last_id]
            let mut count = 0u32;
            let mut size = 0u64;
            let mut j = obj_idx;
            while j < objects.len() && objects[j].0 <= last_id {
                count += 1;
                size += objects[j].1 as u64;
                j += 1;
            }
            intervals.push(TimelineInterval {
                timestamp_us: ts,
                id_from: prev_last_id,
                id_to: last_id,
                count,
                size,
            });
            prev_last_id = last_id;
        }
        self.timeline = intervals;
    }

    /// Returns the allocation timeline intervals, or empty if no samples.
    pub fn get_timeline(&self) -> &[TimelineInterval] {
        &self.timeline
    }

    #[allow(dead_code)]
    pub fn node_count(&self) -> usize {
        self.node_count
    }

    pub fn node_for_snapshot_object_id(&self, id: NodeId) -> Option<NodeOrdinal> {
        let nfc = self.node_field_count;
        let ido = self.node_id_offset;
        for ordinal in 0..self.node_count {
            if self.nodes[ordinal * nfc + ido] as u64 == id.0 {
                return Some(NodeOrdinal(ordinal));
            }
        }
        None
    }

    pub fn node_id(&self, ordinal: NodeOrdinal) -> NodeId {
        NodeId(self.nodes[ordinal.0 * self.node_field_count + self.node_id_offset] as u64)
    }

    pub fn node_self_size(&self, ordinal: NodeOrdinal) -> u32 {
        self.nodes[ordinal.0 * self.node_field_count + self.node_self_size_offset]
    }

    pub fn node_retained_size(&self, ordinal: NodeOrdinal) -> f64 {
        self.retained_sizes[ordinal.0]
    }

    pub fn reachable_size(&self, roots: &[NodeOrdinal]) -> ReachableInfo {
        let nfc = self.node_field_count;
        let efc = self.edge_fields_count;
        let eto = self.edge_to_node_offset;
        let etype_off = self.edge_type_offset;

        let mut visited = vec![false; self.node_count];
        let mut queue = std::collections::VecDeque::with_capacity(roots.len());
        let mut total: f64 = 0.0;
        let mut contexts = Vec::new();

        for &root in roots {
            if !visited[root.0] {
                visited[root.0] = true;
                total += self.node_self_size(root) as f64;
                if self.is_native_context(root) {
                    contexts.push(root);
                }
                queue.push_back(root.0);
            }
        }

        while let Some(ordinal) = queue.pop_front() {
            let first_edge = self.first_edge_indexes[ordinal] as usize;
            let last_edge = self.first_edge_indexes[ordinal + 1] as usize;
            let mut ei = first_edge;
            while ei < last_edge {
                let edge_type = self.edges[ei + etype_off];
                if edge_type == self.edge_weak_type || edge_type == self.edge_shortcut_type {
                    ei += efc;
                    continue;
                }
                let child_ordinal = self.edges[ei + eto] as usize / nfc;
                if child_ordinal == ordinal || visited[child_ordinal] {
                    ei += efc;
                    continue;
                }
                visited[child_ordinal] = true;
                total += self.node_self_size(NodeOrdinal(child_ordinal)) as f64;
                if self.is_native_context(NodeOrdinal(child_ordinal)) {
                    contexts.push(NodeOrdinal(child_ordinal));
                }
                queue.push_back(child_ordinal);
                ei += efc;
            }
        }

        ReachableInfo {
            size: total,
            native_contexts: contexts,
        }
    }

    pub fn node_distance(&self, ordinal: NodeOrdinal) -> Distance {
        self.node_distances[ordinal.0]
    }

    pub fn node_class_name(&self, ordinal: NodeOrdinal) -> String {
        self.strings[self.class_index(ordinal) as usize].clone()
    }

    pub fn node_raw_name(&self, ordinal: NodeOrdinal) -> &str {
        let ni = ordinal.0 * self.node_field_count;
        &self.strings[self.nodes[ni + self.node_name_offset] as usize]
    }

    pub fn is_native_context(&self, ordinal: NodeOrdinal) -> bool {
        let name = self.node_raw_name(ordinal);
        name == "system / NativeContext" || name.starts_with("system / NativeContext / ")
    }

    /// Extracts the URL from a NativeContext node's raw name.
    /// The format is "system / NativeContext / <url>".
    /// Returns the URL portion, or None if not present.
    pub fn native_context_url(&self, ordinal: NodeOrdinal) -> Option<&str> {
        let raw = self.node_raw_name(ordinal);
        let suffix = raw.strip_prefix("system / NativeContext / ")?;
        if suffix.is_empty() {
            None
        } else {
            Some(suffix)
        }
    }

    /// Find the target node of a named internal edge from `ordinal`.
    /// Find a child node by its node name (not edge name).
    pub fn find_child_by_node_name(&self, ordinal: NodeOrdinal, name: &str) -> Option<NodeOrdinal> {
        for (_, child_ord) in self.iter_edges(ordinal) {
            if self.node_raw_name(child_ord) == name {
                return Some(child_ord);
            }
        }
        None
    }

    /// Find the target node of a named internal edge from `ordinal`.
    pub fn find_edge_target(&self, ordinal: NodeOrdinal, name: &str) -> Option<NodeOrdinal> {
        let efc = self.edge_fields_count;
        let nfc = self.node_field_count;
        let first = self.first_edge_indexes[ordinal.0] as usize;
        let last = self.first_edge_indexes[ordinal.0 + 1] as usize;
        let mut ei = first;
        while ei < last {
            let edge_type = self.edges[ei + self.edge_type_offset];
            // Only check string-named edges (not element/hidden which use numeric indices)
            if edge_type != self.edge_element_type && edge_type != self.edge_hidden_type {
                let name_idx = self.edges[ei + self.edge_name_offset] as usize;
                if self.strings[name_idx] == name {
                    let child_index = self.edges[ei + self.edge_to_node_offset] as usize;
                    return Some(NodeOrdinal(child_index / nfc));
                }
            }
            ei += efc;
        }
        None
    }

    /// For a SharedFunctionInfo node, extract its source code from the linked Script.
    /// Returns `None` if start_position or end_position are negative, or if the
    /// script/source chain cannot be resolved.
    pub fn shared_function_info_source(&self, ordinal: NodeOrdinal) -> Option<&str> {
        let raw_name = self.node_raw_name(ordinal);
        if !raw_name.starts_with("system / SharedFunctionInfo") {
            return None;
        }

        let start_pos = self.int_edge_value(ordinal, "start_position")?;
        let end_pos = self.int_edge_value(ordinal, "end_position")?;
        if start_pos < 0 || end_pos < 0 || end_pos < start_pos {
            return None;
        }

        let script_ord = self.find_edge_target(ordinal, "script")?;
        let source_ord = self.find_edge_target(script_ord, "source")?;
        let source = self.node_raw_name(source_ord);

        let start = start_pos as usize;
        let end = end_pos as usize;
        if start <= source.len() && end <= source.len() {
            Some(&source[start..end])
        } else {
            None
        }
    }

    /// Follow the "previous" chain from a Context to find its NativeContext.
    /// Returns `None` if the node is not a Context or the chain doesn't reach
    /// a NativeContext. If the node is already a NativeContext, returns it directly.
    pub fn find_native_context_for_context(&self, ordinal: NodeOrdinal) -> Option<NodeOrdinal> {
        if !self.is_context(ordinal) {
            return None;
        }
        if self.is_native_context(ordinal) {
            return Some(ordinal);
        }
        let mut current = ordinal;
        // Limit iterations to prevent infinite loops on malformed data.
        for _ in 0..self.node_count() {
            match self.find_edge_target(current, "previous") {
                Some(prev) if self.is_native_context(prev) => return Some(prev),
                Some(prev) if self.is_context(prev) => current = prev,
                _ => return None,
            }
        }
        None
    }

    pub fn node_native_context_bucket(&self, ordinal: NodeOrdinal) -> NativeContextBucket {
        self.node_native_context_buckets[ordinal.0]
    }

    /// Returns true if this node is a Context (including NativeContext).
    pub fn is_context(&self, ordinal: NodeOrdinal) -> bool {
        let name = self.node_raw_name(ordinal);
        name.starts_with("system / Context") || name.starts_with("system / NativeContext")
    }

    /// Get the variable names stored in a Context node (context-typed edges, excluding "this").
    pub fn context_variable_names(&self, ordinal: NodeOrdinal) -> Vec<String> {
        let mut vars = Vec::new();
        let efc = self.edge_fields_count;
        let first = self.first_edge_indexes[ordinal.0] as usize;
        let last = self.first_edge_indexes[ordinal.0 + 1] as usize;
        let mut ei = first;
        while ei < last {
            let edge_type = self.edges[ei + self.edge_type_offset];
            if edge_type == self.edge_context_type {
                let name_idx = self.edges[ei + self.edge_name_offset] as usize;
                let name = &self.strings[name_idx];
                if name != "this" {
                    vars.push(name.clone());
                }
            }
            ei += efc;
        }
        vars.sort();
        vars
    }

    /// Returns true if this node is a Script.
    pub fn is_script(&self, ordinal: NodeOrdinal) -> bool {
        self.node_raw_name(ordinal).starts_with("system / Script")
    }

    /// Get the full script source for a Script or SharedFunctionInfo node.
    /// For a Script, follows the "source" edge directly.
    /// For a SharedFunctionInfo, follows "script" -> "source".
    pub fn script_source(&self, ordinal: NodeOrdinal) -> Option<&str> {
        let raw_name = self.node_raw_name(ordinal);
        let script_ord = if raw_name.starts_with("system / Script") {
            ordinal
        } else if raw_name.starts_with("system / SharedFunctionInfo") {
            self.find_edge_target(ordinal, "script")?
        } else {
            return None;
        };
        let source_ord = self.find_edge_target(script_ord, "source")?;
        let source = self.node_raw_name(source_ord);
        if source.is_empty() {
            None
        } else {
            Some(source)
        }
    }

    /// Read the numeric value of an int-typed edge (e.g. start_position -> value).
    pub fn int_edge_value(&self, ordinal: NodeOrdinal, edge_name: &str) -> Option<i64> {
        let int_ord = self.find_edge_target(ordinal, edge_name)?;
        let value_ord = self.find_edge_target(int_ord, "value")?;
        self.node_raw_name(value_ord).parse::<i64>().ok()
    }

    /// Returns the detachedness state of a node: 0=unknown, 1=attached, 2=detached.
    pub fn node_detachedness(&self, ordinal: NodeOrdinal) -> u8 {
        if self.node_detachedness_offset == -1 {
            return 0;
        }
        let ni = ordinal.0 * self.node_field_count;
        (self.nodes[ni + self.node_detachedness_offset as usize] & BITMASK_FOR_DOM_LINK_STATE) as u8
    }

    /// Returns the detachedness of a NativeContext inferred from its global object.
    /// Tries global_object (the Window) first, then global_proxy_object.
    /// Returns: 0=unknown (utility/no global object), 1=attached, 2=detached.
    pub fn native_context_detachedness(&self, ordinal: NodeOrdinal) -> u8 {
        // Try global_object (the Window itself) — propagate_dom_state sets detachedness on it.
        if let Some(go) = self.find_edge_target(ordinal, "global_object") {
            let d = self.node_detachedness(go);
            if d != 0 {
                return d;
            }
        }
        // Fall back to global_proxy_object.
        if let Some(gp) = self.find_edge_target(ordinal, "global_proxy_object") {
            let d = self.node_detachedness(gp);
            if d != 0 {
                return d;
            }
        }
        0
    }

    /// Returns a display label for a NativeContext: URL plus a local frame-kind
    /// heuristic.
    ///
    /// V8 heap snapshots expose the embedder-supplied NativeContext tag
    /// (typically a URL or thread/context name), but not an explicit window
    /// classification. We infer:
    /// - `main` if the context has a Window global object and the global proxy
    ///   looks large
    /// - `iframe` if it has a Window global object but the proxy looks small
    /// - `utility` if there is no Window global object
    pub fn native_context_label(&self, ordinal: NodeOrdinal) -> String {
        let node_id = self.node_id(ordinal);
        let url = self.native_context_url(ordinal);
        let frame_kind = self
            .native_context_data(ordinal)
            .map(|ctx| ctx.kind)
            .unwrap_or_else(|| self.compute_native_context_kind(ordinal))
            .as_str();

        match url {
            Some(u) => format!("[{frame_kind}] {u} @{node_id}"),
            None => format!("[{frame_kind}] @{node_id}"),
        }
    }

    /// Returns the display name for a node, matching Chrome DevTools' JSHeapSnapshotNode.name().
    /// For concatenated strings, follows the cons string chain.
    /// For plain Objects, builds a {prop1, prop2, ...} style name from properties.
    /// For everything else, returns the raw name.
    pub fn node_display_name(&self, ordinal: NodeOrdinal) -> String {
        let ni = ordinal.0 * self.node_field_count;
        let raw_type = self.nodes[ni + self.node_type_offset];

        if raw_type == self.node_cons_string_type {
            return self.cons_string_name(ordinal);
        }

        if raw_type == self.node_object_type {
            let raw_name = self.node_raw_name(ordinal);
            if raw_name == "Object" {
                return self.plain_object_name(ordinal);
            }
        }

        if raw_type == self.node_number_type {
            let raw_name = self.node_raw_name(ordinal);
            if raw_name == "smi number" || raw_name == "heap number" {
                if let Some(value_ord) = self.find_edge_target(ordinal, "value") {
                    let prefix = if raw_name == "smi number" {
                        "smi"
                    } else {
                        "double"
                    };
                    return format!("{prefix} {}", self.node_raw_name(value_ord));
                }
            }
        }

        Self::normalize_display_name(self.node_raw_name(ordinal))
    }

    /// Returns true if this cons string has been "flattened" by V8, meaning
    /// one of its two parts (`first` or `second`) is the empty string.
    fn is_flat_cons_string(&self, ordinal: NodeOrdinal) -> bool {
        let ni = ordinal.0 * self.node_field_count;
        if self.nodes[ni + self.node_type_offset] != self.node_cons_string_type {
            return false;
        }
        let efc = self.edge_fields_count;
        let begin = self.first_edge_indexes[ordinal.0] as usize;
        let end = self.first_edge_indexes[ordinal.0 + 1] as usize;
        let mut ei = begin;
        while ei < end {
            let edge_type = self.edges[ei + self.edge_type_offset];
            if edge_type == self.edge_internal_type {
                let name_idx = self.edges[ei + self.edge_name_offset] as usize;
                let edge_name = &self.strings[name_idx];
                if edge_name == "first" || edge_name == "second" {
                    let child_index = self.edges[ei + self.edge_to_node_offset] as usize;
                    let child_name_idx = self.nodes[child_index + self.node_name_offset] as usize;
                    if self.strings[child_name_idx].is_empty() {
                        return true;
                    }
                }
            }
            ei += efc;
        }
        false
    }

    fn cons_string_name(&self, ordinal: NodeOrdinal) -> String {
        let nfc = self.node_field_count;
        let efc = self.edge_fields_count;
        let mut stack: Vec<usize> = vec![ordinal.0 * nfc];
        let mut name = String::new();

        while let Some(node_index) = stack.pop() {
            if name.len() >= 1024 {
                break;
            }
            let node_type = self.nodes[node_index + self.node_type_offset];
            if node_type != self.node_cons_string_type {
                let name_idx = self.nodes[node_index + self.node_name_offset] as usize;
                name.push_str(&self.strings[name_idx]);
                continue;
            }
            let node_ordinal = node_index / nfc;
            let begin = self.first_edge_indexes[node_ordinal] as usize;
            let end = self.first_edge_indexes[node_ordinal + 1] as usize;
            let mut first_node_index: Option<usize> = None;
            let mut second_node_index: Option<usize> = None;
            let mut ei = begin;
            while ei < end && (first_node_index.is_none() || second_node_index.is_none()) {
                let edge_type = self.edges[ei + self.edge_type_offset];
                if edge_type == self.edge_internal_type {
                    let name_idx = self.edges[ei + self.edge_name_offset] as usize;
                    let edge_name = &self.strings[name_idx];
                    if edge_name == "first" {
                        first_node_index = Some(self.edges[ei + self.edge_to_node_offset] as usize);
                    } else if edge_name == "second" {
                        second_node_index =
                            Some(self.edges[ei + self.edge_to_node_offset] as usize);
                    }
                }
                ei += efc;
            }
            if let Some(idx) = second_node_index {
                stack.push(idx);
            }
            if let Some(idx) = first_node_index {
                stack.push(idx);
            }
        }
        name
    }

    fn plain_object_name(&self, ordinal: NodeOrdinal) -> String {
        let efc = self.edge_fields_count;
        let first_edge = self.first_edge_indexes[ordinal.0] as usize;
        let last_edge = self.first_edge_indexes[ordinal.0 + 1] as usize;

        let mut category_name_start = "{".to_string();
        let mut category_name_end = "}".to_string();
        let mut edge_index_from_start = first_edge;
        let mut edge_index_from_end = if last_edge >= efc {
            last_edge - efc
        } else {
            first_edge
        };
        let mut next_from_end = false;

        while edge_index_from_start <= edge_index_from_end {
            let ei = if next_from_end {
                edge_index_from_end
            } else {
                edge_index_from_start
            };
            let edge_type = self.edges[ei + self.edge_type_offset];

            // Skip non-property edges and __proto__
            if edge_type != self.edge_property_type {
                if next_from_end {
                    if edge_index_from_end < efc {
                        break;
                    }
                    edge_index_from_end -= efc;
                } else {
                    edge_index_from_start += efc;
                }
                continue;
            }
            let name_idx = self.edges[ei + self.edge_name_offset] as usize;
            let edge_name = &self.strings[name_idx];
            if edge_name == "__proto__" {
                if next_from_end {
                    if edge_index_from_end < efc {
                        break;
                    }
                    edge_index_from_end -= efc;
                } else {
                    edge_index_from_start += efc;
                }
                continue;
            }

            let formatted = Self::format_property_name_display(edge_name);

            // Always include at least one property. Beyond that, stop if too long.
            if category_name_start.chars().count() > 1
                && category_name_start.chars().count()
                    + category_name_end.chars().count()
                    + formatted.chars().count()
                    > 100
            {
                break;
            }

            if next_from_end {
                if edge_index_from_end < efc {
                    break;
                }
                edge_index_from_end -= efc;
                if category_name_end.len() > 1 {
                    category_name_end = format!(", {}", category_name_end);
                }
                category_name_end = format!("{}{}", formatted, category_name_end);
            } else {
                edge_index_from_start += efc;
                if category_name_start.len() > 1 {
                    category_name_start.push_str(", ");
                }
                category_name_start.push_str(&formatted);
            }
            next_from_end = !next_from_end;
        }

        if edge_index_from_start <= edge_index_from_end {
            category_name_start.push_str(", \u{2026}");
        }
        if category_name_end.len() > 1 {
            category_name_start.push_str(", ");
        }
        format!("{}{}", category_name_start, category_name_end)
    }

    fn format_property_name_display(name: &str) -> String {
        if name.contains(',')
            || name.contains('\'')
            || name.contains('"')
            || name.contains('{')
            || name.contains('}')
        {
            return Self::json_escape_string(name);
        }
        name.to_string()
    }

    fn json_escape_string(s: &str) -> String {
        let mut result = String::with_capacity(s.len() + 2);
        result.push('"');
        for ch in s.chars() {
            match ch {
                '"' => result.push_str("\\\""),
                '\\' => result.push_str("\\\\"),
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                c if (c as u32) < 0x20 => {
                    result.push_str(&format!("\\u{:04x}", c as u32));
                }
                c => result.push(c),
            }
        }
        result.push('"');
        result
    }

    pub fn node_edge_count(&self, ordinal: NodeOrdinal) -> u32 {
        self.nodes[ordinal.0 * self.node_field_count + self.node_edge_count_offset]
    }

    pub fn is_root(&self, ordinal: NodeOrdinal) -> bool {
        ordinal.0 == self.gc_roots_ordinal
    }

    /// Returns true when `ordinal` is directly retained by `(GC roots)`,
    /// i.e. it is a root category such as `(Strong roots)` or `(Handle scope)`.
    pub fn is_root_holder(&self, ordinal: NodeOrdinal) -> bool {
        let nfc = self.node_field_count;
        let begin = self.first_retainer_index[ordinal.0] as usize;
        let end = self.first_retainer_index[ordinal.0 + 1] as usize;
        for idx in begin..end {
            let node_index = self.retaining_nodes[idx] as usize;
            let ret_ordinal = NodeOrdinal(node_index / nfc);
            if self.is_root(ret_ordinal) {
                return true;
            }
        }
        false
    }

    pub fn edge_type_name(&self, edge_index: usize) -> &str {
        let t = self.edges[edge_index + self.edge_type_offset] as usize;
        if t < self.edge_types.len() {
            &self.edge_types[t]
        } else {
            "unknown"
        }
    }

    pub fn edge_name(&self, edge_index: usize) -> String {
        let edge_type = self.edges[edge_index + self.edge_type_offset];
        let name_or_index = self.edges[edge_index + self.edge_name_offset];

        // Element and hidden edges use numeric index
        if edge_type == self.edge_element_type || edge_type == self.edge_hidden_type {
            return name_or_index.to_string();
        }
        // String-based edges
        self.strings[name_or_index as usize].clone()
    }

    /// Format a node label: `@id name` (or just `@id` if name is empty)
    pub fn format_node_label(&self, ordinal: NodeOrdinal) -> String {
        let name = self.node_display_name(ordinal);
        let id = self.node_id(ordinal);
        if name.is_empty() {
            format!("@{id}")
        } else {
            format!("@{id} {name}")
        }
    }

    /// Format the edge name portion, bracketing element/hidden edges.
    fn format_edge_name(&self, edge_idx: usize) -> String {
        let edge_name = self.edge_name(edge_idx);
        let edge_type = self.edge_type_name(edge_idx);
        if edge_type == "element" || edge_type == "hidden" {
            format!("[{edge_name}]")
        } else if edge_name.is_empty() {
            "??".to_string()
        } else {
            edge_name
        }
    }

    /// Format an outgoing edge label: `edge :: @id name`
    pub fn format_edge_label(&self, edge_idx: usize, child_ord: NodeOrdinal) -> String {
        let edge = self.format_edge_name(edge_idx);
        let node = self.format_node_label(child_ord);
        format!("{edge} :: {node}")
    }

    /// Format a retainer edge label: `edge in @id name`
    pub fn format_retainer_label(&self, edge_idx: usize, ret_ord: NodeOrdinal) -> String {
        let edge = self.format_edge_name(edge_idx);
        let node = self.format_node_label(ret_ord);
        format!("{edge} in {node}")
    }

    pub fn is_invisible_edge(&self, edge_index: usize) -> bool {
        self.edges[edge_index + self.edge_type_offset] == self.edge_invisible_type
    }

    /// Returns a zero-allocation iterator over the outgoing edges of `ordinal`.
    /// Each item is `(edge_index, child_ordinal)`.
    pub fn iter_edges(&self, ordinal: NodeOrdinal) -> EdgeIter<'_> {
        let first = self.first_edge_indexes[ordinal.0] as usize;
        let last = self.first_edge_indexes[ordinal.0 + 1] as usize;
        EdgeIter {
            edges: &self.edges,
            edge_fields_count: self.edge_fields_count,
            edge_to_node_offset: self.edge_to_node_offset,
            node_field_count: self.node_field_count,
            current: first,
            end: last,
        }
    }

    pub fn retainer_count(&self, ordinal: NodeOrdinal) -> usize {
        let begin = self.first_retainer_index[ordinal.0] as usize;
        let end = self.first_retainer_index[ordinal.0 + 1] as usize;
        end - begin
    }

    pub fn get_retainers(&self, ordinal: NodeOrdinal) -> Vec<(usize, NodeOrdinal)> {
        let nfc = self.node_field_count;
        let begin = self.first_retainer_index[ordinal.0] as usize;
        let end = self.first_retainer_index[ordinal.0 + 1] as usize;
        let mut result = Vec::new();
        for idx in begin..end {
            let edge_index = self.retaining_edges[idx] as usize;
            let node_index = self.retaining_nodes[idx] as usize;
            let node_ordinal = NodeOrdinal(node_index / nfc);
            result.push((edge_index, node_ordinal));
        }
        result
    }

    pub fn for_each_retainer<F>(&self, ordinal: NodeOrdinal, mut f: F)
    where
        F: FnMut(usize, NodeOrdinal),
    {
        let nfc = self.node_field_count;
        let begin = self.first_retainer_index[ordinal.0] as usize;
        let end = self.first_retainer_index[ordinal.0 + 1] as usize;
        for idx in begin..end {
            let edge_index = self.retaining_edges[idx] as usize;
            let node_index = self.retaining_nodes[idx] as usize;
            let node_ordinal = NodeOrdinal(node_index / nfc);
            f(edge_index, node_ordinal);
        }
    }

    /// Returns the immediate dominator of `ordinal` in the dominator tree.
    pub fn dominator_of(&self, ordinal: NodeOrdinal) -> NodeOrdinal {
        NodeOrdinal(self.dominators_tree[ordinal.0] as usize)
    }

    pub fn get_dominated_children(&self, ordinal: NodeOrdinal) -> Vec<NodeOrdinal> {
        let nfc = self.node_field_count;
        let from = self.first_dominated_node_index[ordinal.0] as usize;
        let to = self.first_dominated_node_index[ordinal.0 + 1] as usize;
        (from..to)
            .map(|i| NodeOrdinal(self.dominated_nodes[i] as usize / nfc))
            .collect()
    }

    pub fn get_statistics(&self) -> &Statistics {
        &self.statistics
    }

    pub fn aggregates_with_filter(&self) -> AggregateMap {
        let mut aggregates = self.build_aggregates(|_| true);
        self.calculate_classes_retained_size(&mut aggregates);
        for agg in aggregates.values_mut() {
            let rs = &self.retained_sizes;
            agg.node_ordinals
                .sort_by(|a, b| rs[b.0].partial_cmp(&rs[a.0]).unwrap());
        }
        aggregates
    }

    /// BFS from the root, skipping edges where `skip_edge` returns true.
    /// Returns a bitmap where `true` means the node is NOT reachable under
    /// these constraints (i.e. only retained via skipped edges).
    fn compute_retained_bitmap(
        &self,
        skip_edge: impl Fn(usize, usize, usize) -> bool, // (edge_idx, source_ord, target_ord)
    ) -> Vec<bool> {
        let nfc = self.node_field_count;
        let efc = self.edge_fields_count;
        let eto = self.edge_to_node_offset;
        let root = self.root_node_index / nfc;

        let mut reachable = vec![false; self.node_count];
        reachable[root] = true;

        let mut queue = std::collections::VecDeque::new();
        queue.push_back(root);

        while let Some(ord) = queue.pop_front() {
            let first = self.first_edge_indexes[ord] as usize;
            let last = self.first_edge_indexes[ord + 1] as usize;
            let mut ei = first;
            while ei < last {
                let child_ord = self.edges[ei + eto] as usize / nfc;
                if !reachable[child_ord] && !skip_edge(ei, ord, child_ord) {
                    reachable[child_ord] = true;
                    queue.push_back(child_ord);
                }
                ei += efc;
            }
        }

        // "Retained by X" means: reachable in normal graph, but NOT reachable
        // when skipping X. Truly unreachable nodes (not reachable even normally)
        // should not appear.
        let normal_reachable = {
            let mut nr = vec![false; self.node_count];
            nr[root] = true;
            let mut q = std::collections::VecDeque::new();
            q.push_back(root);
            while let Some(ord) = q.pop_front() {
                let first = self.first_edge_indexes[ord] as usize;
                let last = self.first_edge_indexes[ord + 1] as usize;
                let mut ei = first;
                while ei < last {
                    let child_ord = self.edges[ei + eto] as usize / nfc;
                    if !nr[child_ord] {
                        nr[child_ord] = true;
                        q.push_back(child_ord);
                    }
                    ei += efc;
                }
            }
            nr
        };

        // Retained = normally reachable AND not reachable with filter
        reachable
            .iter()
            .zip(normal_reachable.iter())
            .map(|(&filtered_reachable, &normally_reachable)| {
                normally_reachable && !filtered_reachable
            })
            .collect()
    }

    /// Objects only retained by detached DOM nodes.
    pub fn retained_by_detached_dom(&self) -> AggregateMap {
        if self.node_detachedness_offset < 0 {
            return FxHashMap::default();
        }
        let det_off = self.node_detachedness_offset as usize;
        let nfc = self.node_field_count;
        let retained = self.compute_retained_bitmap(|_ei, _src, target| {
            self.nodes[target * nfc + det_off] & BITMASK_FOR_DOM_LINK_STATE == 2
        });
        self.build_aggregates(|ordinal| retained[ordinal])
    }

    /// Objects only retained by DevTools console references.
    pub fn retained_by_console(&self) -> AggregateMap {
        let nfc = self.node_field_count;
        let retained = self.compute_retained_bitmap(|ei, src, _target| {
            let src_type = self.nodes[src * nfc + self.node_type_offset];
            if src_type != self.node_synthetic_type {
                return false;
            }
            self.edge_name(ei).ends_with(" / DevTools console")
        });
        self.build_aggregates(|ordinal| retained[ordinal])
    }

    /// Objects only retained by event handler functions.
    pub fn retained_by_event_handlers(&self) -> AggregateMap {
        let nfc = self.node_field_count;
        let nmo = self.node_name_offset;

        // Step 1: identify event handler nodes via V8EventListener -> callback_object_
        let mut is_handler = vec![false; self.node_count];

        for ordinal in 0..self.node_count {
            let name = &self.strings[self.nodes[ordinal * nfc + nmo] as usize];
            if name != "V8EventListener" {
                continue;
            }
            for (edge_idx, callback_ord) in self.iter_edges(NodeOrdinal(ordinal)) {
                if self.edge_name(edge_idx) != "callback_object_" {
                    continue;
                }
                // Direct handler: callback has a "code" edge
                if self
                    .iter_edges(callback_ord)
                    .any(|(ei, _)| self.edge_name(ei) == "code")
                {
                    is_handler[callback_ord.0] = true;
                    continue;
                }
                // Framework wrapper: a child of callback has a "code" edge
                let mut found = false;
                for (_, child_ord) in self.iter_edges(callback_ord) {
                    if self
                        .iter_edges(child_ord)
                        .any(|(ei, _)| self.edge_name(ei) == "code")
                    {
                        is_handler[child_ord.0] = true;
                        found = true;
                        break;
                    }
                }
                if !found {
                    is_handler[callback_ord.0] = true;
                }
            }
        }

        // Step 2: BFS skipping handler nodes
        let retained = self.compute_retained_bitmap(|_ei, _src, target| is_handler[target]);
        self.build_aggregates(|ordinal| retained[ordinal])
    }

    /// Build aggregates for unreachable nodes only (distance >= UNREACHABLE_BASE).
    pub fn unreachable_aggregates(&self) -> AggregateMap {
        self.build_aggregates(|ordinal| self.node_distances[ordinal].is_unreachable())
    }

    /// Build aggregates for fully unreachable nodes only (distance == UNREACHABLE_BASE).
    pub fn unreachable_root_aggregates(&self) -> AggregateMap {
        self.build_aggregates(|ordinal| self.node_distances[ordinal].is_unreachable_root())
    }

    /// Build aggregates for objects whose ID falls in (id_from, id_to].
    pub fn aggregates_for_id_range(&self, id_from: u64, id_to: u64) -> AggregateMap {
        let nfc = self.node_field_count;
        let ido = self.node_id_offset;
        self.build_aggregates(|ordinal| {
            let id = self.nodes[ordinal * nfc + ido] as u64;
            id > id_from && id <= id_to
        })
    }

    fn build_aggregates(&self, filter: impl Fn(usize) -> bool) -> AggregateMap {
        let nfc = self.node_field_count;
        let sso = self.node_self_size_offset;

        let mut aggregates: FxHashMap<ClassKey, AggregateInfo> = FxHashMap::default();
        let mut next_first_seen: u32 = 0;

        for ordinal in 0..self.node_count {
            let node_index = ordinal * nfc;
            let self_size = self.nodes[node_index + sso] as f64;
            if self_size == 0.0 {
                continue;
            }
            if !filter(ordinal) {
                continue;
            }

            let node_ordinal = NodeOrdinal(ordinal);
            let class_key = self.class_key_internal(node_ordinal);
            let distance = self.node_distances[ordinal];
            let class_name = self.node_class_name(node_ordinal);

            aggregates
                .entry(class_key)
                .and_modify(|agg| {
                    agg.distance = agg.distance.min(distance);
                    agg.count += 1;
                    agg.self_size += self_size;
                    agg.node_ordinals.push(node_ordinal);
                })
                .or_insert_with(|| {
                    let fs = next_first_seen;
                    next_first_seen += 1;
                    AggregateInfo {
                        count: 1,
                        distance,
                        self_size,
                        max_ret: 0.0,
                        name: class_name,
                        first_seen: fs,
                        node_ordinals: vec![node_ordinal],
                    }
                });
        }

        // Convert to string-keyed map
        let mut result: AggregateMap = FxHashMap::default();
        for (key, agg) in aggregates {
            let str_key = match key {
                ClassKey::Index(idx) => self.strings[idx as usize].clone(),
                ClassKey::Location(sid, line, col, ref name) => {
                    format!("{sid},{line},{col},{name}")
                }
            };
            result.insert(str_key, agg);
        }
        result
    }

    fn calculate_classes_retained_size(&self, aggregates: &mut AggregateMap) {
        let nfc = self.node_field_count;

        let mut list: Vec<usize> = vec![self.gc_roots_ordinal * nfc];
        let mut sizes: Vec<i64> = vec![-1];
        let mut class_keys: Vec<String> = Vec::new();
        let mut seen_class_keys: FxHashMap<String, bool> = FxHashMap::default();

        while let Some(node_index) = list.pop() {
            let ordinal = node_index / nfc;
            let class_key = self.class_key_string(NodeOrdinal(ordinal));
            let seen = *seen_class_keys.get(&class_key).unwrap_or(&false);
            let dom_from = self.first_dominated_node_index[ordinal] as usize;
            let dom_to = self.first_dominated_node_index[ordinal + 1] as usize;

            if !seen && self.nodes[ordinal * nfc + self.node_self_size_offset] > 0 {
                if let Some(agg) = aggregates.get_mut(&class_key) {
                    agg.max_ret += self.retained_sizes[ordinal];
                }
                if dom_from != dom_to {
                    seen_class_keys.insert(class_key.clone(), true);
                    sizes.push(list.len() as i64);
                    class_keys.push(class_key.clone());
                }
            }

            for i in dom_from..dom_to {
                list.push(self.dominated_nodes[i] as usize);
            }

            let l = list.len() as i64;
            while !sizes.is_empty() && *sizes.last().unwrap() == l {
                sizes.pop();
                if let Some(ck) = class_keys.pop() {
                    seen_class_keys.insert(ck, false);
                }
            }
        }
    }

    /// Find duplicate strings in the heap. Groups string nodes by their display
    /// name and returns entries with count >= 2, sorted by wasted bytes descending.
    pub fn duplicate_strings(&self) -> Vec<DuplicateStringInfo> {
        let nfc = self.node_field_count;
        let mut groups: FxHashMap<String, DuplicateStringInfo> = FxHashMap::default();

        for ordinal in 0..self.node_count {
            let ni = ordinal * nfc;
            let raw_type = self.nodes[ni + self.node_type_offset];
            if raw_type != self.node_string_type && raw_type != self.node_cons_string_type {
                continue;
            }

            // Skip flattened cons strings — V8 internal artifacts where one
            // part is empty.  Reporting these as duplicates of their own
            // content is just noise.
            if raw_type == self.node_cons_string_type
                && self.is_flat_cons_string(NodeOrdinal(ordinal))
            {
                continue;
            }

            let display_name = self.node_display_name(NodeOrdinal(ordinal));
            if display_name.is_empty() {
                continue;
            }

            let self_size = self.nodes[ni + self.node_self_size_offset] as f64;
            groups
                .entry(display_name.clone())
                .and_modify(|e| {
                    e.count += 1;
                    e.total_size += self_size;
                })
                .or_insert(DuplicateStringInfo {
                    value: display_name,
                    count: 1,
                    instance_size: self_size,
                    total_size: self_size,
                });
        }

        let mut result: Vec<DuplicateStringInfo> =
            groups.into_values().filter(|e| e.count >= 2).collect();
        result.sort_by(|a, b| {
            b.wasted_size()
                .partial_cmp(&a.wasted_size())
                .unwrap()
                .then(b.count.cmp(&a.count))
        });
        result
    }

    fn class_key_string(&self, ordinal: NodeOrdinal) -> String {
        match self.class_key_internal(ordinal) {
            ClassKey::Index(idx) => self.strings[idx as usize].clone(),
            ClassKey::Location(sid, line, col, name) => {
                format!("{sid},{line},{col},{name}")
            }
        }
    }
}

pub struct EdgeIter<'a> {
    edges: &'a [u32],
    edge_fields_count: usize,
    edge_to_node_offset: usize,
    node_field_count: usize,
    current: usize,
    end: usize,
}

impl<'a> Iterator for EdgeIter<'a> {
    type Item = (usize, NodeOrdinal);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.end {
            return None;
        }
        let ei = self.current;
        let child_index = self.edges[ei + self.edge_to_node_offset] as usize;
        let child_ordinal = NodeOrdinal(child_index / self.node_field_count);
        self.current += self.edge_fields_count;
        Some((ei, child_ordinal))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = if self.current >= self.end {
            0
        } else {
            (self.end - self.current) / self.edge_fields_count
        };
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for EdgeIter<'a> {}

#[derive(Hash, Eq, PartialEq, Clone)]
enum ClassKey {
    Index(u32),
    Location(u32, u32, u32, String),
}

#[cfg(test)]
mod tests;
