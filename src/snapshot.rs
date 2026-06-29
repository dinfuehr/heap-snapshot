// Copyright 2011 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// This file was started from Chromium DevTools' HeapSnapshot.ts
// (front_end/entrypoints/heap_snapshot_worker/HeapSnapshot.ts).

use std::sync::OnceLock;

use regex::Regex;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::types::{
    AggregateInfo, AggregateMap, DuplicateStringInfo, DuplicateStringsResult, EdgeId, EdgeRecord,
    NodeId, NodeOrdinal, NodeRecord, SnapshotHeader, Statistics,
};
use crate::utils::{utf16_offset_to_byte, utf16_offset_to_line_column};

mod load;

pub const V8_STACK_ROOTS: &str = "(Stack roots)";
pub const CPPGC_STACK_ROOTS: &str = "C++ native stack roots";

use crate::types::Distance;
const BITMASK_FOR_DOM_LINK_STATE: u32 = 0b11;
const BITMAP_WORD_BITS: usize = u64::BITS as usize;
const INVALID_NODE_ORDINAL: usize = usize::MAX;

#[derive(Clone, Debug)]
struct Bitmap {
    words: Vec<u64>,
}

impl Bitmap {
    fn new(len: usize) -> Self {
        Self {
            words: vec![0; len.div_ceil(BITMAP_WORD_BITS)],
        }
    }

    fn set(&mut self, index: usize) {
        self.words[index / BITMAP_WORD_BITS] |= 1u64 << (index % BITMAP_WORD_BITS);
    }

    fn get(&self, index: usize) -> bool {
        (self.words[index / BITMAP_WORD_BITS] & (1u64 << (index % BITMAP_WORD_BITS))) != 0
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct RetainedByContext {
    context_count: u32,
    retained_by_context_size: u64,
    not_retained_by_context_size: u64,
}

struct RetainedByContextReachability {
    context_count: u32,
    context_retention: Vec<ContextRetention>,
}

struct DominatorData {
    // Total bytes retained by each node, indexed by node ordinal.
    retained_sizes: Vec<u64>,
    // Immediate dominator for each node, indexed by node ordinal.
    immediate_dominators: Vec<u32>,
    // Flat adjacency list of dominated children. Children for node `n` live in
    // `dominated_nodes[first_dominated_node_index[n]..first_dominated_node_index[n + 1]]`.
    dominated_nodes: Vec<u32>,
    // Prefix offsets into `dominated_nodes`; length is node_count + 1.
    first_dominated_node_index: Vec<u32>,
}

impl DominatorData {
    fn empty() -> Self {
        Self {
            retained_sizes: Vec::new(),
            immediate_dominators: Vec::new(),
            dominated_nodes: Vec::new(),
            first_dominated_node_index: Vec::new(),
        }
    }
}

struct NativeContextAttributionData {
    // Best-effort owner bucket for each node: a specific NativeContext, shared
    // by multiple contexts, or unattributed.
    node_native_context_buckets: Vec<NativeContextBucket>,
    // Sum of node self sizes for each NativeContext bucket.
    native_context_sizes: Vec<u64>,
    // Sum of node self sizes in the shared NativeContext bucket.
    shared_attributable_size: u64,
    // Sum of node self sizes that could not be attributed to a NativeContext.
    unattributed_size: u64,
}

impl NativeContextAttributionData {
    fn empty() -> Self {
        Self {
            node_native_context_buckets: Vec::new(),
            native_context_sizes: Vec::new(),
            shared_attributable_size: 0,
            unattributed_size: 0,
        }
    }
}

struct RetainedByContextData {
    // Aggregate result of the "block ordinary Context objects" coverage pass.
    retained_by_context: RetainedByContext,
    // Per-node coverage classification used by summary filters.
    context_retention: Vec<ContextRetention>,
}

impl RetainedByContextData {
    fn empty() -> Self {
        Self {
            retained_by_context: RetainedByContext::default(),
            context_retention: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ContextRetention {
    Retained,
    NotRetained,
    Unreachable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RootKind {
    NonRoot = 0,
    SyntheticRoot = 1,
    SystemRoot = 2,
    UserRoot = 3,
}

/// DOM link state for a node, as propagated by `propagate_detachedness`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Detachedness {
    Unknown = 0,
    Attached = 1,
    Detached = 2,
}

fn decode_detachedness(raw: u32) -> Detachedness {
    match raw & BITMASK_FOR_DOM_LINK_STATE {
        1 => Detachedness::Attached,
        2 => Detachedness::Detached,
        _ => Detachedness::Unknown,
    }
}

#[derive(Clone, Debug, Default)]
pub struct SnapshotOptions {
    /// Treat weak edges as reachable when computing distances.
    /// Objects referenced only via weak edges get distance+1 of the
    /// retainer instead of being marked unreachable (U).
    pub weak_is_reachable: bool,
}

struct ParsedHeapSnapshot {
    snapshot: SnapshotHeader,
    nodes: Vec<NodeRecord>,
    edges: Vec<EdgeRecord>,
    strings: Vec<String>,
    locations: Vec<u32>,
    trace_function_infos: Vec<u32>,
    trace_tree_parents: Vec<u32>,
    trace_tree_func_idxs: Vec<u32>,
    samples: Vec<u32>,
}

#[cfg(test)]
impl ParsedHeapSnapshot {
    fn from_raw_parts(
        snapshot: SnapshotHeader,
        raw_nodes: Vec<u32>,
        raw_edges: Vec<u32>,
        strings: Vec<String>,
        locations: Vec<u32>,
        trace_function_infos: Vec<u32>,
        trace_tree_parents: Vec<u32>,
        trace_tree_func_idxs: Vec<u32>,
        samples: Vec<u32>,
    ) -> Self {
        let meta = &snapshot.meta;

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

        let nodes = Self::build_node_records(
            &raw_nodes,
            node_field_count,
            node_type_offset,
            node_name_offset,
            node_id_offset,
            node_self_size_offset,
            node_edge_count_offset,
            node_detachedness_offset,
            node_trace_node_id_offset,
        );
        let edges = Self::build_edge_records(
            &raw_edges,
            edge_fields_count,
            edge_type_offset,
            edge_name_offset,
            edge_to_node_offset,
            node_field_count,
        );

        Self {
            snapshot,
            nodes,
            edges,
            strings,
            locations,
            trace_function_infos,
            trace_tree_parents,
            trace_tree_func_idxs,
            samples,
        }
    }

    fn build_node_records(
        raw_nodes: &[u32],
        node_field_count: usize,
        node_type_offset: usize,
        node_name_offset: usize,
        node_id_offset: usize,
        node_self_size_offset: usize,
        node_edge_count_offset: usize,
        node_detachedness_offset: i32,
        node_trace_node_id_offset: i32,
    ) -> Vec<NodeRecord> {
        let node_count = raw_nodes.len() / node_field_count;
        let mut records = Vec::with_capacity(node_count);
        let mut first_edge = 0u32;
        for ordinal in 0..node_count {
            let node_index = ordinal * node_field_count;
            let edge_count = raw_nodes[node_index + node_edge_count_offset];
            records.push(NodeRecord {
                type_id: raw_nodes[node_index + node_type_offset],
                name: raw_nodes[node_index + node_name_offset],
                id: raw_nodes[node_index + node_id_offset],
                self_size: raw_nodes[node_index + node_self_size_offset],
                edge_count,
                detachedness: if node_detachedness_offset >= 0 {
                    raw_nodes[node_index + node_detachedness_offset as usize]
                } else {
                    0
                },
                trace_node_id: if node_trace_node_id_offset >= 0 {
                    raw_nodes[node_index + node_trace_node_id_offset as usize]
                } else {
                    0
                },
                first_edge,
            });
            first_edge += edge_count;
        }
        records
    }

    fn build_edge_records(
        raw_edges: &[u32],
        edge_fields_count: usize,
        edge_type_offset: usize,
        edge_name_offset: usize,
        edge_to_node_offset: usize,
        node_field_count: usize,
    ) -> Vec<EdgeRecord> {
        let edge_count = raw_edges.len() / edge_fields_count;
        let mut records = Vec::with_capacity(edge_count);
        for edge_ordinal in 0..edge_count {
            let edge_index = edge_ordinal * edge_fields_count;
            records.push(EdgeRecord {
                type_id: raw_edges[edge_index + edge_type_offset],
                name_or_index: raw_edges[edge_index + edge_name_offset],
                to_node_ordinal: (raw_edges[edge_index + edge_to_node_offset] as usize
                    / node_field_count) as u32,
                _padding: 0,
            });
        }
        records
    }
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
    pub size: u64,
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
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeContextAttributableSizes {
    pub native_contexts: Vec<NativeContextData>,
    pub shared: u64,
    pub unattributed: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NativeContextId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeContextBucket {
    Context(NativeContextId),
    Shared,
    Unattributed,
}

/// A single `@<id>` reference extracted from a V8 edge name (e.g. the
/// three references embedded in a WeakMap ephemeron edge label).
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedEdgeRef {
    /// Short human label for the role/type, e.g. `"key/HTMLDivElement"`,
    /// `"value/Object"`, `"table"`.
    pub label: String,
    pub id: NodeId,
}

fn weak_map_ephemeron_edge_regex() -> &'static Regex {
    static WEAK_MAP_EPHEMERON_EDGE_RE: OnceLock<Regex> = OnceLock::new();
    WEAK_MAP_EPHEMERON_EDGE_RE.get_or_init(|| {
        Regex::new(
            r"^\d+( / part of key \(.*? @\d+\) -> value \(.*? @\d+\) pair in WeakMap \(table @(\d+)\))$",
        )
        .unwrap()
    })
}

/// Parse `@<id>` references out of a V8 edge name. Currently recognizes the
/// WeakMap ephemeron format (built by `SetNamedAutoIndexReference` +
/// `ExtractEphemeronHashTableReferences` in V8's heap-snapshot-generator.cc):
///   `"<index> / part of key (TYPE @N) -> value (TYPE @N) pair in WeakMap (table @N)"`
/// Returns one entry per `@<id>` reference in the order they appear, or an
/// empty vector if the name does not match a recognized format.
pub fn parse_edge_refs(edge_name: &str) -> Vec<ParsedEdgeRef> {
    static EPHEMERON_RE: OnceLock<Regex> = OnceLock::new();
    let re = EPHEMERON_RE.get_or_init(|| {
        Regex::new(
            r"^\d+ / part of key \((.+?) @(\d+)\) -> value \((.+?) @(\d+)\) pair in WeakMap \(table @(\d+)\)$",
        )
        .unwrap()
    });
    let Some(caps) = re.captures(edge_name) else {
        return Vec::new();
    };
    let parse_id = |i: usize| -> Option<NodeId> { caps[i].parse::<u64>().ok().map(NodeId) };
    let (Some(key_id), Some(value_id), Some(table_id)) = (parse_id(2), parse_id(4), parse_id(5))
    else {
        return Vec::new();
    };
    vec![
        ParsedEdgeRef {
            label: format!("key/{}", &caps[1]),
            id: key_id,
        },
        ParsedEdgeRef {
            label: format!("value/{}", &caps[3]),
            id: value_id,
        },
        ParsedEdgeRef {
            label: "table".to_string(),
            id: table_id,
        },
    ]
}

pub struct HeapSnapshot {
    // Cooked graph data. Node and edge handles are ordinals into these arrays.
    nodes: Vec<NodeRecord>,
    edges: Vec<EdgeRecord>,
    strings: Vec<String>,

    // Field metadata still needed after parsing.
    node_field_count: usize,
    node_detachedness_offset: i32,  // -1 if not present
    node_trace_node_id_offset: i32, // -1 if not present

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
    root_ordinal: usize,
    gc_roots_ordinal: usize,
    node_distances: Vec<Distance>,
    dominator_data: DominatorData,

    // Retainers
    retaining_nodes: Vec<u32>,
    retaining_edges: Vec<u32>,
    first_retainer_index: Vec<u32>,

    // Root classification for each node ordinal.
    root_kinds: Vec<RootKind>,
    system_roots: Vec<NodeOrdinal>,
    user_roots: Vec<NodeOrdinal>,

    // Class index per node
    class_indices: Vec<u32>,

    // Detachedness per node. All Unknown when the snapshot does not carry a
    // detachedness field.
    detachedness: Vec<Detachedness>,

    // Location map: node ordinal -> SourceLocation
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
    native_context_attribution: NativeContextAttributionData,
    // Ordinary Context reachability coverage used by summary filters and stats.
    retained_by_context: RetainedByContextData,
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
    extra_native_bytes: u64,

    // When true, weak edges are treated as reachable during BFS distance
    // computation.  Objects referenced only via weak edges from reachable
    // nodes get distance+1 of the retainer instead of being marked
    // unreachable (U).
    weak_is_reachable: bool,

    // Lazily computed bitmaps for retained-by summary filters. The snapshot is
    // immutable after construction, so these stay valid for the lifetime of the
    // snapshot and can be shared by the web, TUI, CLI, and MCP summary paths.
    normal_reachable_bitmap: OnceLock<Bitmap>,
    retained_by_detached_dom_bitmap: OnceLock<Bitmap>,
    retained_by_console_bitmap: OnceLock<Bitmap>,
    retained_by_event_handlers_bitmap: OnceLock<Bitmap>,

    // Statistics (computed at init)
    statistics: Statistics,
}

impl HeapSnapshot {
    #[inline]
    fn first_edge_index(&self, ordinal: usize) -> usize {
        if ordinal == self.node_count {
            self.edge_count
        } else {
            self.nodes[ordinal].first_edge as usize
        }
    }

    #[inline]
    fn node_edge_range(&self, ordinal: usize) -> (usize, usize) {
        let node = &self.nodes[ordinal];
        let first = node.first_edge as usize;
        (first, first + node.edge_count as usize)
    }

    #[inline]
    fn root_ordinal(&self) -> usize {
        self.root_ordinal
    }

    #[inline]
    fn node_type_raw(&self, ordinal: usize) -> u32 {
        self.nodes[ordinal].type_id
    }

    #[inline]
    fn node_name_index(&self, ordinal: usize) -> usize {
        self.nodes[ordinal].name as usize
    }

    #[inline]
    fn edge_type_raw(&self, edge_ordinal: usize) -> u32 {
        self.edges[edge_ordinal].type_id
    }

    #[inline]
    fn edge_name_or_index(&self, edge_ordinal: usize) -> u32 {
        self.edges[edge_ordinal].name_or_index
    }

    #[inline]
    fn edge_to_node_ordinal(&self, edge_ordinal: usize) -> usize {
        self.edges[edge_ordinal].to_node_ordinal as usize
    }

    fn has_global_tag(name: &str, tag: &str) -> bool {
        let bytes = name.as_bytes();
        let tag_len = tag.len();

        if name.ends_with(tag) && bytes.len() > tag_len && bytes[bytes.len() - tag_len - 1] == b' '
        {
            return true;
        }

        name.match_indices(tag).any(|(idx, _)| {
            idx > 0 && bytes[idx - 1] == b' ' && name[idx + tag_len..].starts_with(" / ")
        })
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

    fn calculate_array_size(&self, ordinal: NodeOrdinal) -> u64 {
        let mut size = self.nodes[ordinal.0].self_size as u64;

        let first_edge = self.first_edge_index(ordinal.0);
        let last_edge = self.first_edge_index(ordinal.0 + 1);
        let mut ei = first_edge;
        while ei < last_edge {
            let et = self.edge_type_raw(ei);
            if et != self.edge_internal_type {
                ei += 1;
                continue;
            }
            // Check if edge name is "elements"
            let name_idx = self.edge_name_or_index(ei) as usize;
            if self.strings[name_idx] != "elements" {
                ei += 1;
                continue;
            }
            let elements_ordinal = self.edge_to_node_ordinal(ei);
            // Check retainers count
            let ret_count = self.first_retainer_index[elements_ordinal + 1]
                - self.first_retainer_index[elements_ordinal];
            if ret_count == 1 {
                size += self.nodes[elements_ordinal].self_size as u64;
            }
            break;
        }
        size
    }

    fn class_index(&self, ordinal: NodeOrdinal) -> u32 {
        self.class_indices[ordinal.0]
    }

    fn class_key_internal(&self, ordinal: NodeOrdinal) -> ClassKey {
        let raw_type = self.node_type_raw(ordinal.0);
        if raw_type != self.node_object_type {
            return ClassKey::Index(self.class_index(ordinal));
        }
        if let Some(&loc) = self.location_map.get(&ordinal.0) {
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

    fn dominator_data(&self) -> &DominatorData {
        &self.dominator_data
    }

    fn detachedness(&self) -> &[Detachedness] {
        &self.detachedness
    }

    fn native_context_attribution_data(&self) -> &NativeContextAttributionData {
        &self.native_context_attribution
    }

    fn retained_by_context_data(&self) -> &RetainedByContextData {
        &self.retained_by_context
    }

    // Public API

    pub fn native_contexts(&self) -> &[NativeContextData] {
        &self.native_contexts
    }

    pub fn native_context_by_id(&self, id: NativeContextId) -> &NativeContextData {
        &self.native_contexts()[id.0 as usize]
    }

    pub fn native_context_id(&self, ordinal: NodeOrdinal) -> Option<NativeContextId> {
        self.native_contexts()
            .iter()
            .position(|ctx| ctx.ordinal == ordinal)
            .map(|idx| NativeContextId(idx as u32))
    }

    pub fn native_context_data(&self, ordinal: NodeOrdinal) -> Option<&NativeContextData> {
        self.native_contexts()
            .iter()
            .find(|ctx| ctx.ordinal == ordinal)
    }

    pub fn native_context_attributable_sizes(&self) -> NativeContextAttributableSizes {
        NativeContextAttributableSizes {
            native_contexts: self.native_contexts().to_vec(),
            shared: self.shared_attributable_size(),
            unattributed: self.unattributed_size(),
        }
    }

    pub fn native_context_attributable_size(&self, ordinal: NodeOrdinal) -> Option<u64> {
        self.native_context_id(ordinal)
            .map(|id| self.native_context_by_id(id).size)
    }

    pub fn shared_attributable_size(&self) -> u64 {
        self.native_context_attribution_data()
            .shared_attributable_size
    }

    pub fn unattributed_size(&self) -> u64 {
        self.native_context_attribution_data().unattributed_size
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

    /// Returns sorted variable names from a NativeContext's ScriptContextTable.
    /// These are `let`/`const` declarations at the top-level script scope.
    fn script_context_table_vars(&self, table: NodeOrdinal) -> Vec<String> {
        let mut vars = Vec::new();
        // The ScriptContextTable has hidden edges to Context objects.
        for (edge_idx, child_ord) in self.iter_edges(table) {
            let edge_type = self.edge_type_raw(edge_idx.0);
            if edge_type != self.edge_hidden_type && edge_type != self.edge_element_type {
                continue;
            }
            // Each Context has "context"-typed edges for its variables.
            for (ctx_edge_idx, _) in self.iter_edges(child_ord) {
                let ctx_edge_type = self.edge_type_raw(ctx_edge_idx.0);
                if ctx_edge_type != self.edge_context_type {
                    continue;
                }
                let name_idx = self.edge_name_or_index(ctx_edge_idx.0) as usize;
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
        Self::has_global_tag(self.node_raw_name(ordinal), "[JSGlobalObject]")
    }

    pub fn is_js_global_proxy(&self, ordinal: NodeOrdinal) -> bool {
        Self::has_global_tag(self.node_raw_name(ordinal), "[JSGlobalProxy]")
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
        debug_assert_ne!(self.gc_roots_ordinal, INVALID_NODE_ORDINAL);
        NodeOrdinal(self.gc_roots_ordinal)
    }

    /// The snapshot's synthetic root (node 0).  Use this for views like
    /// containment that want to show `(GC roots)` as a visible child.
    pub fn synthetic_root_ordinal(&self) -> NodeOrdinal {
        NodeOrdinal(self.root_ordinal())
    }

    /// Returns true when `ordinal` is a user root — a non-synthetic direct
    /// child of the synthetic root (typically a NativeContext).
    pub fn is_user_root(&self, ordinal: NodeOrdinal) -> bool {
        self.root_kinds[ordinal.0] == RootKind::UserRoot
    }

    pub fn root_kind(&self, ordinal: NodeOrdinal) -> RootKind {
        self.root_kinds[ordinal.0]
    }

    pub fn system_roots(&self) -> &[NodeOrdinal] {
        &self.system_roots
    }

    pub fn user_roots(&self) -> &[NodeOrdinal] {
        &self.user_roots
    }

    #[allow(dead_code)]
    pub fn node_type_name(&self, ordinal: NodeOrdinal) -> &str {
        let t = self.node_type_raw(ordinal.0) as usize;
        if t < self.node_types.len() {
            &self.node_types[t]
        } else {
            "unknown"
        }
    }

    /// Returns the raw name of a string-typed node, or `None` if the node
    /// is not of type "string". This is for synthetic string nodes created
    /// by `AddStringEdge` in V8, where the node name *is* the value.
    pub fn node_value_as_str(&self, ordinal: NodeOrdinal) -> Option<&str> {
        if self.node_type_raw(ordinal.0) != self.node_string_type {
            return None;
        }
        Some(self.node_raw_name(ordinal))
    }

    /// Interprets a number node named "int" as an integer value by following
    /// its "value" edge. Returns `None` if the node is not an int-typed
    /// number node.
    pub fn node_value_as_int(&self, ordinal: NodeOrdinal) -> Option<i64> {
        if self.node_type_raw(ordinal.0) != self.node_number_type {
            return None;
        }
        if self.node_raw_name(ordinal) != "int" {
            return None;
        }
        let val_ord = self.find_edge_target(ordinal, "value")?;
        self.node_raw_name(val_ord).parse::<i64>().ok()
    }

    /// Returns the true character length of a string node by following its
    /// `length` internal edge to an int-typed number node.
    pub fn node_string_length(&self, ordinal: NodeOrdinal) -> Option<u32> {
        let len_ord = self.find_edge_target(ordinal, "length")?;
        let val = self.node_value_as_int(len_ord)?;
        u32::try_from(val).ok()
    }

    /// Returns the V8 string hash by following its `hash` internal edge to an
    /// int-typed number node.
    pub fn node_string_hash(&self, ordinal: NodeOrdinal) -> Option<i64> {
        let hash_ord = self.find_edge_target(ordinal, "hash")?;
        self.node_value_as_int(hash_ord)
    }

    /// Returns true if this string node has a `truncated` internal edge
    /// pointing to a bool `true` node, meaning its display name is only a
    /// prefix of the real content.
    pub fn node_is_truncated_string(&self, ordinal: NodeOrdinal) -> bool {
        self.find_edge_target(ordinal, "truncated")
            .and_then(|t| self.node_value_as_bool(t))
            == Some(true)
    }

    /// Returns true if this string node has a `two_byte_representation`
    /// internal edge pointing to a bool `true` node, meaning it uses
    /// UTF-16 (2 bytes per char).
    pub fn node_is_two_byte_string(&self, ordinal: NodeOrdinal) -> bool {
        self.find_edge_target(ordinal, "two_byte_representation")
            .and_then(|t| self.node_value_as_bool(t))
            == Some(true)
    }

    /// Interprets a number node named "bool" as a boolean value by following
    /// its "value" edge. Returns `None` if the node is not a bool-typed
    /// number node.
    pub fn node_value_as_bool(&self, ordinal: NodeOrdinal) -> Option<bool> {
        if self.node_type_raw(ordinal.0) != self.node_number_type {
            return None;
        }
        if self.node_raw_name(ordinal) != "bool" {
            return None;
        }
        let val_ord = self.find_edge_target(ordinal, "value")?;
        match self.node_raw_name(val_ord) {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
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
        for (&ordinal, loc) in &self.location_map {
            if !needed.contains(&loc.script_id) {
                continue;
            }
            if let Some(name) = self.find_script_name(NodeOrdinal(ordinal)) {
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
        self.node_type_raw(ordinal.0) == self.node_closure_type
    }

    /// Returns true if this node is a SharedFunctionInfo.
    /// SFI nodes have V8 node type "code".
    pub fn is_shared_function_info(&self, ordinal: NodeOrdinal) -> bool {
        if self.node_type_raw(ordinal.0) != self.node_code_type {
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
        if let Some(&loc) = self.location_map.get(&ordinal.0) {
            return Some(loc);
        }
        if self.is_js_function(ordinal) {
            for (edge_idx, child_ord) in self.iter_edges(ordinal) {
                if self.edge_name(edge_idx) == "shared" {
                    if let Some(&loc) = self.location_map.get(&child_ord.0) {
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

    /// For a JSFunction or SharedFunctionInfo, returns the (line, column) of
    /// the function's `end_position`, computed by scanning the script source.
    /// Both `end_position` and the returned column are in UTF-16 code units,
    /// matching V8's source-position convention.
    ///
    /// Unlike the start location — which V8 resolves into (line, column) via
    /// `Script::GetPositionInfo` and writes into the snapshot's top-level
    /// `locations` array (consumed by `node_location`) — the end position is
    /// emitted only as a raw `end_position` integer edge on the SFI. So we
    /// convert it ourselves by walking the script source.
    ///
    /// Returns `None` if the end position is not available or the script
    /// source cannot be resolved.
    pub fn function_end_line_column(&self, ordinal: NodeOrdinal) -> Option<(u32, u32)> {
        let sfi_ord = if self.is_js_function(ordinal) {
            self.find_edge_target(ordinal, "shared")?
        } else if self.is_shared_function_info(ordinal) {
            ordinal
        } else {
            return None;
        };

        let end_pos = self.int_edge_value(sfi_ord, "end_position")?;
        if end_pos < 0 {
            return None;
        }
        let script_ord = self.find_edge_target(sfi_ord, "script")?;
        let source_ord = self.find_edge_target(script_ord, "source")?;
        let source = self.node_raw_name(source_ord);
        utf16_offset_to_line_column(source, end_pos as u32)
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
        let trace_id = self.nodes[ordinal.0].trace_node_id as usize;
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
        let mut objects: Vec<(u64, u32)> = Vec::with_capacity(self.node_count);
        for ordinal in 0..self.node_count {
            let id = self.nodes[ordinal].id as u64;
            let size = self.nodes[ordinal].self_size;
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
        for ordinal in 0..self.node_count {
            if self.nodes[ordinal].id as u64 == id.0 {
                return Some(NodeOrdinal(ordinal));
            }
        }
        None
    }

    pub fn node_id(&self, ordinal: NodeOrdinal) -> NodeId {
        NodeId(self.nodes[ordinal.0].id as u64)
    }

    pub fn node_self_size(&self, ordinal: NodeOrdinal) -> u32 {
        self.nodes[ordinal.0].self_size
    }

    pub fn node_retained_size(&self, ordinal: NodeOrdinal) -> u64 {
        self.dominator_data().retained_sizes[ordinal.0]
    }

    pub fn reachable_size(&self, roots: &[NodeOrdinal]) -> ReachableInfo {
        let mut visited = vec![false; self.node_count];
        let mut queue = std::collections::VecDeque::with_capacity(roots.len());
        let mut total: u64 = 0;
        let mut contexts = Vec::new();

        for &root in roots {
            if !visited[root.0] {
                visited[root.0] = true;
                total += self.node_self_size(root) as u64;
                if self.is_native_context(root) {
                    contexts.push(root);
                }
                queue.push_back(root.0);
            }
        }

        while let Some(ordinal) = queue.pop_front() {
            let first_edge = self.first_edge_index(ordinal);
            let last_edge = self.first_edge_index(ordinal + 1);
            let mut ei = first_edge;
            while ei < last_edge {
                let edge_type = self.edge_type_raw(ei);
                if edge_type == self.edge_weak_type || edge_type == self.edge_shortcut_type {
                    ei += 1;
                    continue;
                }
                let child_ordinal = self.edge_to_node_ordinal(ei);
                if child_ordinal == ordinal || visited[child_ordinal] {
                    ei += 1;
                    continue;
                }
                visited[child_ordinal] = true;
                total += self.node_self_size(NodeOrdinal(child_ordinal)) as u64;
                if self.is_native_context(NodeOrdinal(child_ordinal)) {
                    contexts.push(NodeOrdinal(child_ordinal));
                }
                queue.push_back(child_ordinal);
                ei += 1;
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
        &self.strings[self.node_name_index(ordinal.0)]
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
        let first = self.first_edge_index(ordinal.0);
        let last = self.first_edge_index(ordinal.0 + 1);
        let mut ei = first;
        while ei < last {
            let edge_type = self.edge_type_raw(ei);
            // Only check string-named edges (not element/hidden which use numeric indices)
            if edge_type != self.edge_element_type && edge_type != self.edge_hidden_type {
                let name_idx = self.edge_name_or_index(ei) as usize;
                if self.strings[name_idx] == name {
                    return Some(NodeOrdinal(self.edge_to_node_ordinal(ei)));
                }
            }
            ei += 1;
        }
        None
    }

    fn find_internal_edge_target(&self, ordinal: NodeOrdinal, name: &str) -> Option<NodeOrdinal> {
        let first = self.first_edge_index(ordinal.0);
        let last = self.first_edge_index(ordinal.0 + 1);
        let mut ei = first;
        while ei < last {
            if self.edge_type_raw(ei) == self.edge_internal_type {
                let name_idx = self.edge_name_or_index(ei) as usize;
                if self.strings[name_idx] == name {
                    return Some(NodeOrdinal(self.edge_to_node_ordinal(ei)));
                }
            }
            ei += 1;
        }
        None
    }

    /// Follow an object's `map` edge and read the map's `instance_type_name`.
    pub fn map_instance_type_name(&self, ordinal: NodeOrdinal) -> Option<&str> {
        let map_ord = self.find_internal_edge_target(ordinal, "map")?;
        let type_name_ord = self.find_internal_edge_target(map_ord, "instance_type_name")?;
        self.node_value_as_str(type_name_ord)
            .filter(|type_name| !type_name.is_empty())
    }

    /// For a JSFunction or SharedFunctionInfo node, extract its source code.
    /// Follows `shared` from a JSFunction to reach the SFI, then delegates to
    /// [`Self::shared_function_info_source`].
    pub fn function_source(&self, ordinal: NodeOrdinal) -> Option<&str> {
        if self.is_js_function(ordinal) {
            let sfi = self.find_edge_target(ordinal, "shared")?;
            return self.shared_function_info_source(sfi);
        }
        self.shared_function_info_source(ordinal)
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

        let start = utf16_offset_to_byte(source, start_pos as u32)?;
        let end = utf16_offset_to_byte(source, end_pos as u32)?;
        if start <= end {
            Some(&source[start..end])
        } else {
            None
        }
    }

    /// Follow the "previous" chain from a Context to find its NativeContext.
    /// Returns `None` if the node is not a Context or the chain doesn't reach
    /// a NativeContext. If the node is already a NativeContext, returns it directly.
    pub fn find_native_context_for_context(&self, ordinal: NodeOrdinal) -> Option<NodeOrdinal> {
        if !self.is_context_or_native_context(ordinal) {
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
                Some(prev) if self.is_context_or_native_context(prev) => current = prev,
                _ => return None,
            }
        }
        None
    }

    pub fn node_native_context_bucket(&self, ordinal: NodeOrdinal) -> NativeContextBucket {
        self.native_context_attribution_data()
            .node_native_context_buckets[ordinal.0]
    }

    /// Returns true if this node is an ordinary `system / Context` object.
    pub fn is_context_object(&self, ordinal: NodeOrdinal) -> bool {
        let name = self.node_raw_name(ordinal);
        name == "system / Context" || name.starts_with("system / Context / ")
    }

    /// Returns true if this node is a Context or NativeContext.
    pub fn is_context_or_native_context(&self, ordinal: NodeOrdinal) -> bool {
        self.is_context_object(ordinal) || self.is_native_context(ordinal)
    }

    /// Get the variable names stored in a Context node (context-typed edges, excluding "this").
    pub fn context_variable_names(&self, ordinal: NodeOrdinal) -> Vec<String> {
        let mut vars = Vec::new();
        let first = self.first_edge_index(ordinal.0);
        let last = self.first_edge_index(ordinal.0 + 1);
        let mut ei = first;
        while ei < last {
            let edge_type = self.edge_type_raw(ei);
            if edge_type == self.edge_context_type {
                let name_idx = self.edge_name_or_index(ei) as usize;
                let name = &self.strings[name_idx];
                if name != "this" {
                    vars.push(name.clone());
                }
            }
            ei += 1;
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

    pub fn node_detachedness(&self, ordinal: NodeOrdinal) -> Detachedness {
        self.detachedness()[ordinal.0]
    }

    /// Returns the raw detachedness value encoded in the heap snapshot.
    /// Snapshots without a detachedness field report Unknown.
    pub fn node_original_detachedness(&self, ordinal: NodeOrdinal) -> Detachedness {
        if self.node_detachedness_offset == -1 {
            return Detachedness::Unknown;
        }
        decode_detachedness(self.nodes[ordinal.0].detachedness)
    }

    /// Returns true when the propagated value still matches the original snapshot value.
    pub fn node_detachedness_is_original(&self, ordinal: NodeOrdinal) -> bool {
        self.node_detachedness(ordinal) == self.node_original_detachedness(ordinal)
    }

    /// Returns the detachedness of a NativeContext inferred from its global object.
    /// Tries global_object (the Window) first, then global_proxy_object.
    pub fn native_context_detachedness(&self, ordinal: NodeOrdinal) -> Detachedness {
        // Try global_object (the Window itself) — propagate_detachedness sets detachedness on it.
        if let Some(go) = self.find_edge_target(ordinal, "global_object") {
            let d = self.node_detachedness(go);
            if d != Detachedness::Unknown {
                return d;
            }
        }
        // Fall back to global_proxy_object.
        if let Some(gp) = self.find_edge_target(ordinal, "global_proxy_object") {
            let d = self.node_detachedness(gp);
            if d != Detachedness::Unknown {
                return d;
            }
        }
        Detachedness::Unknown
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
        let ctx_idx = match self.node_native_context_bucket(ordinal) {
            NativeContextBucket::Context(id) => format!(" #{}", id.0),
            _ => String::new(),
        };

        match url {
            Some(u) => format!("[{frame_kind}]{ctx_idx} {u} @{node_id}"),
            None => format!("[{frame_kind}]{ctx_idx} @{node_id}"),
        }
    }

    /// Returns the display name for a node, matching Chrome DevTools' JSHeapSnapshotNode.name().
    /// For concatenated strings, follows the cons string chain.
    /// For plain Objects, builds a {prop1, prop2, ...} style name from properties.
    /// For everything else, returns the raw name.
    pub fn node_display_name(&self, ordinal: NodeOrdinal) -> String {
        let raw_type = self.node_type_raw(ordinal.0);

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
            if let Some(value_ord) = self.find_edge_target(ordinal, "value") {
                let prefix = if raw_name == "smi number" {
                    "smi"
                } else if raw_name == "heap number" {
                    "double"
                } else {
                    raw_name
                };
                return format!("{prefix} {}", self.node_raw_name(value_ord));
            }
        }

        if raw_type == self.node_closure_type {
            let raw_name = self.node_raw_name(ordinal);
            let name = if raw_name.is_empty() {
                "<anonymous>"
            } else {
                raw_name
            };
            return format!("{name} [JSFunction]");
        }

        self.node_raw_name(ordinal).to_string()
    }

    /// Returns true if this cons string has been "flattened" by V8, meaning
    /// one of its two parts (`first` or `second`) is the empty string.
    fn is_flat_cons_string(&self, ordinal: NodeOrdinal) -> bool {
        if self.node_type_raw(ordinal.0) != self.node_cons_string_type {
            return false;
        }
        let begin = self.first_edge_index(ordinal.0);
        let end = self.first_edge_index(ordinal.0 + 1);
        let mut ei = begin;
        while ei < end {
            let edge_type = self.edge_type_raw(ei);
            if edge_type == self.edge_internal_type {
                let name_idx = self.edge_name_or_index(ei) as usize;
                let edge_name = &self.strings[name_idx];
                if edge_name == "first" || edge_name == "second" {
                    let child_ordinal = self.edge_to_node_ordinal(ei);
                    let child_name_idx = self.node_name_index(child_ordinal);
                    if self.strings[child_name_idx].is_empty() {
                        return true;
                    }
                }
            }
            ei += 1;
        }
        false
    }

    fn cons_string_name(&self, ordinal: NodeOrdinal) -> String {
        let mut stack: Vec<usize> = vec![ordinal.0];
        let mut name = String::new();

        while let Some(node_ordinal) = stack.pop() {
            if name.len() >= 1024 {
                break;
            }
            let node_type = self.node_type_raw(node_ordinal);
            if node_type != self.node_cons_string_type {
                let name_idx = self.node_name_index(node_ordinal);
                name.push_str(&self.strings[name_idx]);
                continue;
            }
            let begin = self.first_edge_index(node_ordinal);
            let end = self.first_edge_index(node_ordinal + 1);
            let mut first_node_ordinal: Option<usize> = None;
            let mut second_node_ordinal: Option<usize> = None;
            let mut ei = begin;
            while ei < end && (first_node_ordinal.is_none() || second_node_ordinal.is_none()) {
                let edge_type = self.edge_type_raw(ei);
                if edge_type == self.edge_internal_type {
                    let name_idx = self.edge_name_or_index(ei) as usize;
                    let edge_name = &self.strings[name_idx];
                    if edge_name == "first" {
                        first_node_ordinal = Some(self.edge_to_node_ordinal(ei));
                    } else if edge_name == "second" {
                        second_node_ordinal = Some(self.edge_to_node_ordinal(ei));
                    }
                }
                ei += 1;
            }
            if let Some(idx) = second_node_ordinal {
                stack.push(idx);
            }
            if let Some(idx) = first_node_ordinal {
                stack.push(idx);
            }
        }
        name
    }

    fn plain_object_name(&self, ordinal: NodeOrdinal) -> String {
        let first_edge = self.first_edge_index(ordinal.0);
        let last_edge = self.first_edge_index(ordinal.0 + 1);
        if first_edge == last_edge {
            return "{}".to_string();
        }

        let mut category_name_start = "{".to_string();
        let mut category_name_end = "}".to_string();
        let mut edge_index_from_start = first_edge;
        let mut edge_index_from_end = if last_edge > first_edge {
            last_edge - 1
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
            let edge_type = self.edge_type_raw(ei);

            // Skip non-property edges and __proto__
            if edge_type != self.edge_property_type {
                if next_from_end {
                    if edge_index_from_end == 0 {
                        break;
                    }
                    edge_index_from_end -= 1;
                } else {
                    edge_index_from_start += 1;
                }
                continue;
            }
            let name_idx = self.edge_name_or_index(ei) as usize;
            let edge_name = &self.strings[name_idx];
            if edge_name == "__proto__" {
                if next_from_end {
                    if edge_index_from_end == 0 {
                        break;
                    }
                    edge_index_from_end -= 1;
                } else {
                    edge_index_from_start += 1;
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
                if edge_index_from_end == 0 {
                    break;
                }
                edge_index_from_end -= 1;
                if category_name_end.len() > 1 {
                    category_name_end = format!(", {}", category_name_end);
                }
                category_name_end = format!("{}{}", formatted, category_name_end);
            } else {
                edge_index_from_start += 1;
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
        self.nodes[ordinal.0].edge_count
    }

    pub fn is_root(&self, ordinal: NodeOrdinal) -> bool {
        ordinal.0 == self.gc_roots_ordinal
    }

    /// Returns true when `ordinal` is directly retained by `(GC roots)`,
    /// i.e. it is a root category such as `(Strong roots)` or `(Handle scope)`.
    pub fn is_root_holder(&self, ordinal: NodeOrdinal) -> bool {
        let begin = self.first_retainer_index[ordinal.0] as usize;
        let end = self.first_retainer_index[ordinal.0 + 1] as usize;
        for idx in begin..end {
            let ret_ordinal = NodeOrdinal(self.retaining_nodes[idx] as usize);
            if self.is_root(ret_ordinal) {
                return true;
            }
        }
        false
    }

    pub fn edge_type_name(&self, edge: EdgeId) -> &str {
        let t = self.edge_type_raw(edge.0) as usize;
        if t < self.edge_types.len() {
            &self.edge_types[t]
        } else {
            "unknown"
        }
    }

    pub fn edge_name(&self, edge: EdgeId) -> String {
        let edge_type = self.edge_type_raw(edge.0);
        let name_or_index = self.edge_name_or_index(edge.0);

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
    fn format_edge_name(&self, edge: EdgeId) -> String {
        let edge_name = self.edge_name(edge);
        let edge_type = self.edge_type_name(edge);
        if edge_type == "element" || edge_type == "hidden" {
            format!("[{edge_name}]")
        } else if edge_name.is_empty() {
            "??".to_string()
        } else {
            edge_name
        }
    }

    /// Format an outgoing edge label: `edge :: @id name`
    pub fn format_edge_label(&self, edge: EdgeId, child_ord: NodeOrdinal) -> String {
        let edge_str = self.format_edge_name(edge);
        let node = self.format_node_label(child_ord);
        format!("{edge_str} :: {node}")
    }

    /// Format a retainer edge label: `edge in @id name`
    pub fn format_retainer_label(&self, edge: EdgeId, ret_ord: NodeOrdinal) -> String {
        let edge_str = self.format_edge_name(edge);
        let node = self.format_node_label(ret_ord);
        format!("{edge_str} in {node}")
    }

    pub fn is_invisible_edge(&self, edge: EdgeId) -> bool {
        self.edge_type_raw(edge.0) == self.edge_invisible_type
    }

    pub fn is_weak_edge(&self, edge: EdgeId) -> bool {
        self.edge_type_raw(edge.0) == self.edge_weak_type
    }

    /// Returns a zero-allocation iterator over the outgoing edges of `ordinal`.
    /// Each item is `(edge_ordinal, child_ordinal)`.
    pub fn iter_edges(&self, ordinal: NodeOrdinal) -> EdgeIter<'_> {
        let first = self.first_edge_index(ordinal.0);
        let last = self.first_edge_index(ordinal.0 + 1);
        EdgeIter {
            edges: &self.edges,
            current: first,
            end: last,
        }
    }

    pub fn retainer_count(&self, ordinal: NodeOrdinal) -> usize {
        let begin = self.first_retainer_index[ordinal.0] as usize;
        let end = self.first_retainer_index[ordinal.0 + 1] as usize;
        end - begin
    }

    pub fn get_retainers(&self, ordinal: NodeOrdinal) -> Vec<(EdgeId, NodeOrdinal)> {
        let begin = self.first_retainer_index[ordinal.0] as usize;
        let end = self.first_retainer_index[ordinal.0 + 1] as usize;
        let mut result = Vec::new();
        for idx in begin..end {
            let edge_index = EdgeId(self.retaining_edges[idx] as usize);
            let node_ordinal = NodeOrdinal(self.retaining_nodes[idx] as usize);
            result.push((edge_index, node_ordinal));
        }
        result
    }

    pub fn for_each_retainer<F>(&self, ordinal: NodeOrdinal, mut f: F)
    where
        F: FnMut(EdgeId, NodeOrdinal),
    {
        let begin = self.first_retainer_index[ordinal.0] as usize;
        let end = self.first_retainer_index[ordinal.0 + 1] as usize;
        for idx in begin..end {
            let edge_index = EdgeId(self.retaining_edges[idx] as usize);
            let node_ordinal = NodeOrdinal(self.retaining_nodes[idx] as usize);
            f(edge_index, node_ordinal);
        }
    }

    /// Returns the immediate dominator of `ordinal` in the dominator tree.
    pub fn dominator_of(&self, ordinal: NodeOrdinal) -> NodeOrdinal {
        NodeOrdinal(self.dominator_data().immediate_dominators[ordinal.0] as usize)
    }

    pub fn get_dominated_children(&self, ordinal: NodeOrdinal) -> Vec<NodeOrdinal> {
        let dominator_data = self.dominator_data();
        let from = dominator_data.first_dominated_node_index[ordinal.0] as usize;
        let to = dominator_data.first_dominated_node_index[ordinal.0 + 1] as usize;
        (from..to)
            .map(|i| NodeOrdinal(dominator_data.dominated_nodes[i] as usize))
            .collect()
    }

    pub fn get_statistics(&self) -> &Statistics {
        &self.statistics
    }

    fn compute_aggregates(&self, filter: impl Fn(usize) -> bool) -> AggregateMap {
        let (mut aggregates, ord_to_agg) = self.build_aggregates(filter);
        self.calculate_classes_retained_size(&mut aggregates, &ord_to_agg);
        let rs = &self.dominator_data().retained_sizes;
        for agg in aggregates.iter_mut() {
            agg.node_ordinals
                .sort_by(|a, b| rs[b.0].partial_cmp(&rs[a.0]).unwrap());
        }
        aggregates
    }

    pub fn aggregates_with_filter(&self) -> AggregateMap {
        self.compute_aggregates(|_| true)
    }

    pub fn aggregates_attached(&self) -> AggregateMap {
        self.compute_aggregates(|ordinal| {
            self.node_detachedness(NodeOrdinal(ordinal)) == Detachedness::Attached
        })
    }

    pub fn aggregates_detached(&self) -> AggregateMap {
        self.compute_aggregates(|ordinal| {
            self.node_detachedness(NodeOrdinal(ordinal)) == Detachedness::Detached
        })
    }

    pub fn aggregates_for_retained_by_context_objects(&self) -> AggregateMap {
        let context_retention = &self.retained_by_context_data().context_retention;
        self.compute_aggregates(|ordinal| context_retention[ordinal] == ContextRetention::Retained)
    }

    pub fn aggregates_for_not_retained_by_context_objects(&self) -> AggregateMap {
        let context_retention = &self.retained_by_context_data().context_retention;
        self.compute_aggregates(|ordinal| {
            context_retention[ordinal] == ContextRetention::NotRetained
        })
    }

    pub fn aggregates_for_native_context(&self, context_id: NativeContextId) -> AggregateMap {
        let buckets = &self
            .native_context_attribution_data()
            .node_native_context_buckets;
        self.compute_aggregates(|ordinal| {
            buckets[ordinal] == NativeContextBucket::Context(context_id)
        })
    }

    pub fn aggregates_for_shared_context(&self) -> AggregateMap {
        let buckets = &self
            .native_context_attribution_data()
            .node_native_context_buckets;
        self.compute_aggregates(|ordinal| buckets[ordinal] == NativeContextBucket::Shared)
    }

    pub fn aggregates_for_unattributed_context(&self) -> AggregateMap {
        let buckets = &self
            .native_context_attribution_data()
            .node_native_context_buckets;
        self.compute_aggregates(|ordinal| buckets[ordinal] == NativeContextBucket::Unattributed)
    }

    /// BFS from the root, skipping edges where `skip_edge` returns true.
    /// Returns a bitmap where `true` means the node is reachable under these
    /// constraints.
    fn compute_reachable_bitmap(
        &self,
        skip_edge: impl Fn(EdgeId, usize, usize) -> bool, // (edge_idx, source_ord, target_ord)
    ) -> Bitmap {
        let root = self.root_ordinal();

        let mut reachable = Bitmap::new(self.node_count);
        reachable.set(root);

        let mut queue = std::collections::VecDeque::new();
        queue.push_back(root);

        while let Some(ord) = queue.pop_front() {
            let first = self.first_edge_index(ord);
            let last = self.first_edge_index(ord + 1);
            let mut ei = first;
            while ei < last {
                let child_ord = self.edge_to_node_ordinal(ei);
                if !reachable.get(child_ord) && !skip_edge(EdgeId(ei), ord, child_ord) {
                    reachable.set(child_ord);
                    queue.push_back(child_ord);
                }
                ei += 1;
            }
        }

        reachable
    }

    fn normal_reachable_bitmap(&self) -> &Bitmap {
        self.normal_reachable_bitmap
            .get_or_init(|| self.compute_reachable_bitmap(|_, _, _| false))
    }

    /// Returns a bitmap where `true` means the node is normally reachable, but
    /// not reachable when skipping edges/nodes matched by `skip_edge`.
    fn compute_retained_bitmap(
        &self,
        skip_edge: impl Fn(EdgeId, usize, usize) -> bool, // (edge_idx, source_ord, target_ord)
    ) -> Bitmap {
        let reachable = self.compute_reachable_bitmap(skip_edge);

        // "Retained by X" means: reachable in normal graph, but NOT reachable
        // when skipping X. Truly unreachable nodes (not reachable even normally)
        // should not appear.
        let normal_reachable = self.normal_reachable_bitmap();

        // Retained = normally reachable AND not reachable with filter
        let mut retained = Bitmap::new(self.node_count);
        for ordinal in 0..self.node_count {
            if normal_reachable.get(ordinal) && !reachable.get(ordinal) {
                retained.set(ordinal);
            }
        }
        retained
    }

    fn retained_by_detached_dom_bitmap(&self) -> Option<&Bitmap> {
        if self.node_detachedness_offset < 0 {
            return None;
        }
        Some(self.retained_by_detached_dom_bitmap.get_or_init(|| {
            self.compute_retained_bitmap(|_ei, _src, target| {
                self.detachedness()[target] == Detachedness::Detached
            })
        }))
    }

    /// Objects only retained by detached DOM nodes.
    pub fn retained_by_detached_dom(&self) -> AggregateMap {
        let Some(retained) = self.retained_by_detached_dom_bitmap() else {
            return Vec::new();
        };
        self.compute_aggregates(|ordinal| retained.get(ordinal))
    }

    fn retained_by_console_bitmap(&self) -> &Bitmap {
        self.retained_by_console_bitmap.get_or_init(|| {
            self.compute_retained_bitmap(|ei, src, _target| {
                let src_type = self.node_type_raw(src);
                if src_type != self.node_synthetic_type {
                    return false;
                }
                self.edge_name(ei).ends_with(" / DevTools console")
            })
        })
    }

    /// Objects only retained by DevTools console references.
    pub fn retained_by_console(&self) -> AggregateMap {
        let retained = self.retained_by_console_bitmap();
        self.compute_aggregates(|ordinal| retained.get(ordinal))
    }

    fn retained_by_event_handlers_bitmap(&self) -> &Bitmap {
        self.retained_by_event_handlers_bitmap.get_or_init(|| {
            // Step 1: identify event handler nodes via V8EventListener -> callback_object_
            let mut is_handler = Bitmap::new(self.node_count);

            for ordinal in 0..self.node_count {
                let name = &self.strings[self.node_name_index(ordinal)];
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
                        is_handler.set(callback_ord.0);
                        continue;
                    }
                    // Framework wrapper: a child of callback has a "code" edge
                    let mut found = false;
                    for (_, child_ord) in self.iter_edges(callback_ord) {
                        if self
                            .iter_edges(child_ord)
                            .any(|(ei, _)| self.edge_name(ei) == "code")
                        {
                            is_handler.set(child_ord.0);
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        is_handler.set(callback_ord.0);
                    }
                }
            }

            // Step 2: BFS skipping handler nodes
            self.compute_retained_bitmap(|_ei, _src, target| is_handler.get(target))
        })
    }

    /// Objects only retained by event handler functions.
    pub fn retained_by_event_handlers(&self) -> AggregateMap {
        let retained = self.retained_by_event_handlers_bitmap();
        self.compute_aggregates(|ordinal| retained.get(ordinal))
    }

    /// Build aggregates for unreachable nodes only (distance >= UNREACHABLE_BASE).
    pub fn unreachable_aggregates(&self) -> AggregateMap {
        self.compute_aggregates(|ordinal| self.node_distances[ordinal].is_unreachable())
    }

    /// Build aggregates for fully unreachable nodes only (distance == UNREACHABLE_BASE).
    pub fn unreachable_root_aggregates(&self) -> AggregateMap {
        self.compute_aggregates(|ordinal| self.node_distances[ordinal].is_unreachable_root())
    }

    /// Build aggregates for objects whose ID falls in (id_from, id_to].
    pub fn aggregates_for_id_range(&self, id_from: u64, id_to: u64) -> AggregateMap {
        self.compute_aggregates(|ordinal| {
            let id = self.nodes[ordinal].id as u64;
            id > id_from && id <= id_to
        })
    }

    fn build_aggregates(&self, filter: impl Fn(usize) -> bool) -> (AggregateMap, Vec<u32>) {
        let mut aggregates: FxHashMap<ClassKey, (u32, AggregateInfo)> = FxHashMap::default();
        let mut ord_to_agg: Vec<u32> = vec![u32::MAX; self.node_count];
        let mut next_first_seen: u32 = 0;

        for ordinal in 0..self.node_count {
            let self_size = self.nodes[ordinal].self_size as u64;
            if self_size == 0 {
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
                .and_modify(|(idx, agg)| {
                    ord_to_agg[ordinal] = *idx;
                    agg.distance = agg.distance.min(distance);
                    agg.count += 1;
                    agg.self_size += self_size;
                    agg.node_ordinals.push(node_ordinal);
                })
                .or_insert_with(|| {
                    let fs = next_first_seen;
                    ord_to_agg[ordinal] = fs;
                    next_first_seen += 1;
                    (
                        fs,
                        AggregateInfo {
                            count: 1,
                            distance,
                            self_size,
                            max_ret: 0,
                            name: class_name,
                            first_seen: fs,
                            node_ordinals: vec![node_ordinal],
                        },
                    )
                });
        }

        // Resolve location-based names and collect into Vec
        let mut result: AggregateMap = aggregates
            .into_iter()
            .map(|(key, (_, mut agg))| {
                if let ClassKey::Location(sid, line, col, _) = key {
                    let loc = SourceLocation {
                        script_id: sid,
                        line,
                        column: col,
                    };
                    agg.name = format!("{} [{}]", agg.name, self.format_location(&loc));
                }
                agg
            })
            .collect();
        result.sort_by_key(|a| a.first_seen);
        (result, ord_to_agg)
    }

    fn calculate_classes_retained_size(&self, aggregates: &mut AggregateMap, ord_to_agg: &[u32]) {
        let dominator_data = self.dominator_data();
        let mut list: Vec<usize> = vec![self.gc_roots_ordinal];
        let mut sizes: Vec<i64> = vec![-1];
        let mut class_stack: Vec<u32> = Vec::new();
        let mut seen: FxHashSet<u32> = FxHashSet::default();

        while let Some(ordinal) = list.pop() {
            let agg_idx = ord_to_agg[ordinal];
            let is_seen = agg_idx != u32::MAX && seen.contains(&agg_idx);
            let dom_from = dominator_data.first_dominated_node_index[ordinal] as usize;
            let dom_to = dominator_data.first_dominated_node_index[ordinal + 1] as usize;

            if !is_seen && self.nodes[ordinal].self_size > 0 {
                if agg_idx != u32::MAX {
                    aggregates[agg_idx as usize].max_ret += dominator_data.retained_sizes[ordinal];
                }
                if dom_from != dom_to {
                    if agg_idx != u32::MAX {
                        seen.insert(agg_idx);
                    }
                    sizes.push(list.len() as i64);
                    class_stack.push(agg_idx);
                }
            }

            for i in dom_from..dom_to {
                list.push(dominator_data.dominated_nodes[i] as usize);
            }

            let l = list.len() as i64;
            while !sizes.is_empty() && *sizes.last().unwrap() == l {
                sizes.pop();
                if let Some(idx) = class_stack.pop() {
                    seen.remove(&idx);
                }
            }
        }
    }

    /// Find duplicate strings in the heap. Only considers strings that have a
    /// `length` internal edge (added by newer V8 builds). Groups by display
    /// name (and, for truncated strings, also by length and hash) and returns
    /// entries with count >= 2, sorted by wasted bytes descending.
    pub fn duplicate_strings(&self) -> DuplicateStringsResult {
        let mut groups: FxHashMap<String, Vec<DuplicateStringInfo>> = FxHashMap::default();
        let mut skipped_count: u32 = 0;
        let mut skipped_size: u64 = 0;

        for ordinal in 0..self.node_count {
            let raw_type = self.node_type_raw(ordinal);
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

            let self_size = self.nodes[ordinal].self_size as u64;
            if self_size == 0 {
                continue;
            }

            let ord = NodeOrdinal(ordinal);
            let display_name = self.node_display_name(ord);
            if display_name.is_empty() {
                continue;
            }

            // Only consider strings with a `length` edge.
            let length = match self.node_string_length(ord) {
                Some(len) => len,
                None => {
                    skipped_count += 1;
                    skipped_size += self_size;
                    continue;
                }
            };

            let truncated = self.node_is_truncated_string(ord);
            let two_byte = self.node_is_two_byte_string(ord);
            let hash = truncated.then(|| self.node_string_hash(ord)).flatten();

            let node_id = self.node_id(ord);
            let bucket = groups.entry(display_name.clone()).or_default();
            if let Some(entry) = bucket.iter_mut().find(|entry| {
                entry.truncated == truncated
                    && (!truncated || (entry.length == length && entry.hash == hash))
            }) {
                entry.count += 1;
                entry.total_size += self_size;
                entry.node_ids.push(node_id);
            } else {
                bucket.push(DuplicateStringInfo {
                    value: display_name,
                    count: 1,
                    instance_size: self_size,
                    total_size: self_size,
                    length,
                    hash,
                    truncated,
                    two_byte,
                    node_ids: vec![node_id],
                });
            }
        }

        let mut duplicates: Vec<DuplicateStringInfo> = groups
            .into_values()
            .flatten()
            .filter(|e| e.count >= 2)
            .collect();
        duplicates.sort_by(|a, b| {
            b.wasted_size()
                .cmp(&a.wasted_size())
                .then(b.count.cmp(&a.count))
        });
        DuplicateStringsResult {
            duplicates,
            skipped_count,
            skipped_size,
        }
    }

    /// Build aggregates from duplicate string groups so they can be displayed
    /// in the Summary view: one aggregate per duplicate string value, with
    /// `node_ordinals` listing every duplicate instance.
    pub fn aggregates_for_duplicate_strings(&self) -> AggregateMap {
        let result = self.duplicate_strings();
        if result.duplicates.is_empty() {
            return Vec::new();
        }

        let mut id_to_ord: FxHashMap<u64, NodeOrdinal> = FxHashMap::default();
        id_to_ord.reserve(self.node_count);
        for ordinal in 0..self.node_count {
            let id = self.nodes[ordinal].id as u64;
            id_to_ord.insert(id, NodeOrdinal(ordinal));
        }

        result
            .duplicates
            .into_iter()
            .enumerate()
            .map(|(idx, info)| {
                let mut node_ordinals: Vec<NodeOrdinal> = Vec::with_capacity(info.node_ids.len());
                let mut self_size: u64 = 0;
                let mut max_ret: u64 = 0;
                let mut distance = Distance::NONE;
                for nid in &info.node_ids {
                    if let Some(&ord) = id_to_ord.get(&nid.0) {
                        node_ordinals.push(ord);
                        self_size += self.node_self_size(ord) as u64;
                        max_ret += self.node_retained_size(ord);
                        let d = self.node_distances[ord.0];
                        if d < distance {
                            distance = d;
                        }
                    }
                }

                let mut name = info.value;
                let mut tags: Vec<&str> = Vec::new();
                if info.truncated {
                    tags.push("truncated");
                }
                if info.two_byte {
                    tags.push("2-byte");
                }
                if !tags.is_empty() {
                    name = format!("{name} [{}]", tags.join(", "));
                }

                AggregateInfo {
                    count: info.count,
                    distance,
                    self_size,
                    max_ret,
                    name,
                    first_seen: idx as u32,
                    node_ordinals,
                }
            })
            .collect()
    }
}

pub struct EdgeIter<'a> {
    edges: &'a [EdgeRecord],
    current: usize,
    end: usize,
}

impl<'a> Iterator for EdgeIter<'a> {
    type Item = (EdgeId, NodeOrdinal);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.end {
            return None;
        }
        let ei = self.current;
        let child_ordinal = NodeOrdinal(self.edges[ei].to_node_ordinal as usize);
        self.current += 1;
        Some((EdgeId(ei), child_ordinal))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = if self.current >= self.end {
            0
        } else {
            self.end - self.current
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
