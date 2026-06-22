// Copyright 2011 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
#[cfg(not(target_arch = "wasm32"))]
use std::fs::File;
use std::io;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::sync::OnceLock;

use rustc_hash::{FxHashMap, FxHashSet};

use super::*;
#[cfg(test)]
use crate::types::SnapshotHeader;
use crate::types::{Distance, Statistics};

mod parser;

const MAX_INTERFACE_NAME_LENGTH: usize = 60;
const MIN_INTERFACE_PROPERTY_COUNT: usize = 1;
const NO_EDGE_TARGET: u32 = u32::MAX;
const NO_NATIVE_CONTEXT_ID: u32 = u32::MAX;
const SHARED_NATIVE_CONTEXT_ID: u32 = u32::MAX - 1;

// Temporary load-time cache of selected named edge targets, indexed by
// source node ordinal. Each slot is either a target node ordinal or
// `NO_EDGE_TARGET`. This lets init passes use direct array lookups instead
// of repeatedly scanning a node's edges for common names.
#[derive(Clone, Debug)]
struct InitEdgeTargets {
    // NativeContext -> JSGlobalObject.
    global_object: Vec<u32>,
    // NativeContext -> JSGlobalProxy.
    global_proxy_object: Vec<u32>,
    // Object/map -> NativeContext.
    native_context: Vec<u32>,
    // Object -> Map.
    map: Vec<u32>,
    // NativeContext -> ScriptContextTable.
    script_context_table: Vec<u32>,
}

struct NativeJsMetadata {
    native_contexts: Vec<NativeContextData>,
    native_context_global_fields: FxHashSet<String>,
    native_context_vars: FxHashMap<NodeOrdinal, String>,
    js_global_objects: Vec<usize>,
    js_global_proxies: Vec<usize>,
    js_global_object_fields: FxHashSet<String>,
    js_global_proxy_fields: FxHashSet<String>,
}

struct RetainerIndex {
    // Source node ordinal for each incoming edge entry.
    retaining_nodes: Vec<u32>,
    // Edge index for each incoming edge entry, parallel to `retaining_nodes`.
    retaining_edges: Vec<u32>,
    // Prefix offsets into the retainer arrays; incoming retainers for node `n`
    // live in `first_retainer_index[n]..first_retainer_index[n + 1]`.
    first_retainer_index: Vec<u32>,
}

struct NameData {
    class_indices: Vec<u32>,
    appended_strings: Vec<String>,
}

struct InitPhaseData {
    native_metadata: NativeJsMetadata,
    retainer_index: RetainerIndex,
    dominator_data: DominatorData,
    distances: Vec<Distance>,
    detachedness: Vec<Detachedness>,
    native_context_attribution: NativeContextAttributionData,
    retained_by_context: RetainedByContextData,
    names: NameData,
}

impl InitEdgeTargets {
    fn new(node_count: usize) -> Self {
        Self {
            global_object: vec![NO_EDGE_TARGET; node_count],
            global_proxy_object: vec![NO_EDGE_TARGET; node_count],
            native_context: vec![NO_EDGE_TARGET; node_count],
            map: vec![NO_EDGE_TARGET; node_count],
            script_context_table: vec![NO_EDGE_TARGET; node_count],
        }
    }
}

fn edge_target_ordinal(raw: u32) -> Option<NodeOrdinal> {
    (raw != NO_EDGE_TARGET).then_some(NodeOrdinal(raw as usize))
}

fn native_context_id_from_raw(raw: u32) -> Option<NativeContextId> {
    (raw < SHARED_NATIVE_CONTEXT_ID).then_some(NativeContextId(raw))
}

fn maybe_weak_map_ephemeron_edge_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }
    idx > 0 && name[idx..].starts_with(" / part of key (")
}

fn empty_statistics() -> Statistics {
    Statistics {
        total: 0,
        native_total: 0,
        typed_arrays: 0,
        v8heap_total: 0,
        code: 0,
        js_arrays: 0,
        strings: 0,
        system: 0,
        extra_native_bytes: 0,
        unreachable_count: 0,
        unreachable_size: 0,
        context_count: 0,
        retained_by_context_size: 0,
        not_retained_by_context_size: 0,
    }
}

impl HeapSnapshot {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        Self::load_with_options(path, SnapshotOptions::default())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_with_options<P: AsRef<Path>>(
        path: P,
        options: SnapshotOptions,
    ) -> io::Result<Self> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|e| {
            io::Error::new(e.kind(), format!("Failed to open {}: {e}", path.display()))
        })?;
        let parsed = parser::parse(file).map_err(|e| {
            io::Error::new(e.kind(), format!("Failed to parse {}: {e}", path.display()))
        })?;
        Ok(Self::from_parsed_with_options(parsed, options))
    }

    pub fn from_bytes(data: &[u8]) -> io::Result<Self> {
        Self::from_bytes_with_options(data, SnapshotOptions::default())
    }

    pub fn from_bytes_with_options(data: &[u8], options: SnapshotOptions) -> io::Result<Self> {
        let parsed = parser::parse_from_slice(data)?;
        Ok(Self::from_parsed_with_options(parsed, options))
    }

    fn from_parsed_with_options(parsed: ParsedHeapSnapshot, options: SnapshotOptions) -> Self {
        let ParsedHeapSnapshot {
            snapshot,
            nodes,
            edges,
            strings,
            locations,
            trace_function_infos,
            trace_tree_parents,
            trace_tree_func_idxs,
            samples,
        } = parsed;
        let meta = snapshot.meta;

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

        let node_count = nodes.len();
        let edge_count = edges.len();
        let root_ordinal = snapshot.root_index.unwrap_or(0) / node_field_count;
        let extra_native_bytes = snapshot.extra_native_bytes.unwrap_or(0);

        let mut snap = HeapSnapshot {
            nodes,
            edges,
            strings,
            node_field_count,
            node_detachedness_offset,
            node_trace_node_id_offset,
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
            root_ordinal,
            gc_roots_ordinal: INVALID_NODE_ORDINAL,
            node_distances: Vec::new(),
            dominator_data: DominatorData::empty(),
            retaining_nodes: Vec::new(),
            retaining_edges: Vec::new(),
            first_retainer_index: Vec::new(),
            root_kinds: vec![RootKind::NonRoot; node_count],
            system_roots: Vec::new(),
            user_roots: Vec::new(),
            class_indices: vec![0u32; node_count],
            detachedness: Vec::new(),
            native_contexts: Vec::new(),
            native_context_attribution: NativeContextAttributionData::empty(),
            retained_by_context: RetainedByContextData::empty(),
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
            normal_reachable_bitmap: OnceLock::new(),
            retained_by_detached_dom_bitmap: OnceLock::new(),
            retained_by_console_bitmap: OnceLock::new(),
            retained_by_event_handlers_bitmap: OnceLock::new(),
            statistics: empty_statistics(),
        };

        let init_edge_targets = snap.build_init_edge_targets();

        // Find (GC roots) ordinal — must happen after edge indexes are built
        snap.gc_roots_ordinal = match snap.find_gc_roots_ordinal() {
            Some(ord) => ord,
            None => {
                let root_ord = snap.root_ordinal();
                let first = snap.first_edge_index(root_ord);
                let last = snap.first_edge_index(root_ord + 1);
                let mut children = Vec::new();
                let mut ei = first;
                while ei < last {
                    let child_ordinal = snap.edge_to_node_ordinal(ei);
                    let name = &snap.strings[snap.node_name_index(child_ordinal)];
                    children.push(name.clone());
                    ei += 1;
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
            let root_ord = snap.root_ordinal();
            snap.root_kinds[root_ord] = RootKind::SyntheticRoot;
            let first = snap.first_edge_index(root_ord);
            let last = snap.first_edge_index(root_ord + 1);
            let mut ei = first;
            while ei < last {
                let child_ordinal = snap.edge_to_node_ordinal(ei);
                let child_type = snap.node_type_raw(child_ordinal);
                if child_type != snap.node_synthetic_type {
                    // Non-synthetic child of the synthetic root is a user root.
                    snap.root_kinds[child_ordinal] = RootKind::UserRoot;
                    snap.user_roots.push(NodeOrdinal(child_ordinal));
                } else {
                    let name = &snap.strings[snap.node_name_index(child_ordinal)];
                    if name == "(Document DOM trees)" {
                        // "(Document DOM trees)" is synthetic but treated as a user root.
                        snap.root_kinds[child_ordinal] = RootKind::UserRoot;
                        snap.user_roots.push(NodeOrdinal(child_ordinal));
                    } else {
                        snap.root_kinds[child_ordinal] = RootKind::SystemRoot;
                        snap.system_roots.push(NodeOrdinal(child_ordinal));
                    }
                }
                ei += 1;
            }
        }
        // NOTE: DevTools calls calculateEffectiveSizes here, which
        // transfers shallow sizes from hidden/array/ExternalStringData nodes
        // to their unique non-synthetic owner.  This makes summary view sizes
        // more meaningful by attributing internal backing stores (e.g. the
        // FixedArray behind an Array) to the owning JS object.  We skip this
        // because it only runs when user roots (NativeContexts) are present,
        // and our target snapshots don't use them.
        let init_data = snap.compute_init_phase_data(&init_edge_targets);

        snap.native_contexts = init_data.native_metadata.native_contexts;
        snap.native_context_global_fields = init_data.native_metadata.native_context_global_fields;
        snap.native_context_vars = init_data.native_metadata.native_context_vars;
        snap.js_global_objects = init_data.native_metadata.js_global_objects;
        snap.js_global_proxies = init_data.native_metadata.js_global_proxies;
        snap.js_global_object_fields = init_data.native_metadata.js_global_object_fields;
        snap.js_global_proxy_fields = init_data.native_metadata.js_global_proxy_fields;

        snap.retaining_nodes = init_data.retainer_index.retaining_nodes;
        snap.retaining_edges = init_data.retainer_index.retaining_edges;
        snap.first_retainer_index = init_data.retainer_index.first_retainer_index;

        snap.dominator_data = init_data.dominator_data;
        snap.node_distances = init_data.distances;
        snap.detachedness = init_data.detachedness;
        for (ctx, size) in snap.native_contexts.iter_mut().zip(
            init_data
                .native_context_attribution
                .native_context_sizes
                .iter()
                .copied(),
        ) {
            ctx.size = size;
        }
        snap.native_context_attribution = init_data.native_context_attribution;
        snap.retained_by_context = init_data.retained_by_context;
        snap.class_indices = init_data.names.class_indices;
        snap.strings.extend(init_data.names.appended_strings);

        // Calculate depths within the unreachable subgraph
        snap.calculate_unreachable_depths();

        // Build location map
        {
            let mut map = FxHashMap::default();
            let mut i = 0;
            while i < locations.len() {
                let node_index = locations[i + snap.location_index_offset] as usize;
                let node_ordinal = node_index / snap.node_field_count;
                map.insert(
                    node_ordinal,
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
        if !trace_tree_parents.is_empty() {
            snap.trace_parents = trace_tree_parents;
            snap.trace_func_idxs = trace_tree_func_idxs;
            snap.build_trace_functions(&trace_function_infos, &meta);
        }

        // Build allocation timeline from samples
        if !samples.is_empty() {
            snap.build_timeline(&samples, &meta);
        }

        snap.statistics = snap.calculate_statistics();

        snap
    }

    #[cfg(test)]
    pub(crate) fn from_raw_parts_with_options_for_test(
        snapshot: SnapshotHeader,
        raw_nodes: Vec<u32>,
        raw_edges: Vec<u32>,
        strings: Vec<String>,
        locations: Vec<u32>,
        trace_function_infos: Vec<u32>,
        trace_tree_parents: Vec<u32>,
        trace_tree_func_idxs: Vec<u32>,
        samples: Vec<u32>,
        options: SnapshotOptions,
    ) -> Self {
        let parsed = ParsedHeapSnapshot::from_raw_parts(
            snapshot,
            raw_nodes,
            raw_edges,
            strings,
            locations,
            trace_function_infos,
            trace_tree_parents,
            trace_tree_func_idxs,
            samples,
        );
        Self::from_parsed_with_options(parsed, options)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn compute_init_phase_data(&self, edge_targets: &InitEdgeTargets) -> InitPhaseData {
        std::thread::scope(|scope| {
            let native_context_attribution_worker = scope.spawn(|| {
                let native_metadata = self.compute_native_js_metadata(edge_targets);
                let native_context_attribution = self.compute_native_context_attribution_data(
                    edge_targets,
                    &native_metadata.native_contexts,
                );
                (native_metadata, native_context_attribution)
            });
            let names = scope.spawn(|| self.compute_name_data());
            let detachedness = scope.spawn(|| self.compute_detachedness());
            let retained_by_context_worker = scope.spawn(|| {
                let distances = self.calculate_distances_output();
                let retained_by_context = self.compute_retained_by_context_data(&distances);
                (distances, retained_by_context)
            });
            let retainers = scope.spawn(|| self.build_retainers_output());
            let essential_edges = scope.spawn(|| self.init_essential_edges());

            let essential_edges = essential_edges.join().unwrap();
            let dominator_data = scope.spawn(move || {
                let (retainer_index, dominator_data) =
                    self.calculate_dominator_data(&essential_edges, || retainers.join().unwrap());
                (retainer_index, dominator_data)
            });

            let (native_metadata, native_context_attribution) =
                native_context_attribution_worker.join().unwrap();
            let (distances, retained_by_context) = retained_by_context_worker.join().unwrap();
            let (retainer_index, dominator_data) = dominator_data.join().unwrap();

            InitPhaseData {
                native_metadata,
                retainer_index,
                dominator_data,
                distances,
                detachedness: detachedness.join().unwrap(),
                native_context_attribution,
                retained_by_context,
                names: names.join().unwrap(),
            }
        })
    }

    #[cfg(target_arch = "wasm32")]
    fn compute_init_phase_data(&self, edge_targets: &InitEdgeTargets) -> InitPhaseData {
        let native_metadata = self.compute_native_js_metadata(edge_targets);
        let names = self.compute_name_data();
        let detachedness = self.compute_detachedness();
        let distances = self.calculate_distances_output();
        let retainer_index = self.build_retainers_output();
        let essential_edges = self.init_essential_edges();
        let (retainer_index, dominator_data) =
            self.calculate_dominator_data(&essential_edges, || retainer_index);
        let native_context_attribution = self.compute_native_context_attribution_data(
            edge_targets,
            &native_metadata.native_contexts,
        );
        let retained_by_context = self.compute_retained_by_context_data(&distances);

        InitPhaseData {
            native_metadata,
            retainer_index,
            dominator_data,
            distances,
            detachedness,
            native_context_attribution,
            retained_by_context,
            names,
        }
    }

    fn compute_native_js_metadata(&self, edge_targets: &InitEdgeTargets) -> NativeJsMetadata {
        let native_contexts = self.compute_native_contexts(edge_targets);
        let native_context_global_fields = Self::native_context_global_fields();
        let native_context_vars = self.compute_native_context_vars(
            &native_contexts,
            &native_context_global_fields,
            edge_targets,
        );
        let (js_global_objects, js_global_proxies) = self.find_js_globals_output();
        let js_global_object_fields = self.common_edge_names(&js_global_objects);
        let js_global_proxy_fields = self.common_edge_names(&js_global_proxies);

        NativeJsMetadata {
            native_contexts,
            native_context_global_fields,
            native_context_vars,
            js_global_objects,
            js_global_proxies,
            js_global_object_fields,
            js_global_proxy_fields,
        }
    }

    fn compute_native_contexts(&self, edge_targets: &InitEdgeTargets) -> Vec<NativeContextData> {
        let mut native_contexts = Vec::new();
        for ordinal in 0..self.node_count {
            let ordinal = NodeOrdinal(ordinal);
            if self.is_native_context(ordinal) {
                native_contexts.push(NativeContextData {
                    ordinal,
                    kind: self.compute_native_context_kind_from_targets(ordinal, edge_targets),
                    is_extension: self
                        .native_context_url(ordinal)
                        .is_some_and(|url| url.starts_with("chrome-extension://")),
                    size: 0,
                });
            }
        }

        native_contexts
            .sort_by_key(|ctx| (ctx.is_extension, ctx.kind.sort_priority(), ctx.ordinal.0));
        native_contexts
    }

    fn compute_native_context_kind_from_targets(
        &self,
        ordinal: NodeOrdinal,
        edge_targets: &InitEdgeTargets,
    ) -> NativeContextKind {
        let is_frame = edge_target_ordinal(edge_targets.global_object[ordinal.0])
            .is_some_and(|go| self.node_raw_name(go).starts_with("Window"));

        if !is_frame {
            return NativeContextKind::Utility;
        }

        match edge_target_ordinal(edge_targets.global_proxy_object[ordinal.0]) {
            Some(gp) if self.node_edge_count(gp) >= 10 => NativeContextKind::Main,
            _ => NativeContextKind::Iframe,
        }
    }

    pub(super) fn compute_native_context_kind(&self, ordinal: NodeOrdinal) -> NativeContextKind {
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

    fn compute_native_context_attribution_data(
        &self,
        edge_targets: &InitEdgeTargets,
        native_contexts: &[NativeContextData],
    ) -> NativeContextAttributionData {
        let node_native_context_buckets =
            self.compute_node_native_context_buckets_output(edge_targets, native_contexts);
        let (native_context_sizes, shared_attributable_size, unattributed_size) = self
            .compute_native_context_attributable_sizes_output(
                native_contexts.len(),
                &node_native_context_buckets,
            );

        NativeContextAttributionData {
            node_native_context_buckets,
            native_context_sizes,
            shared_attributable_size,
            unattributed_size,
        }
    }

    fn compute_retained_by_context_data(
        &self,
        node_distances: &[Distance],
    ) -> RetainedByContextData {
        let (retained_by_context, context_retention) =
            self.compute_retained_by_context_output(node_distances);

        RetainedByContextData {
            retained_by_context,
            context_retention,
        }
    }

    fn compute_node_native_context_buckets_output(
        &self,
        edge_targets: &InitEdgeTargets,
        native_contexts: &[NativeContextData],
    ) -> Vec<NativeContextBucket> {
        let mut context_index_by_ordinal = FxHashMap::default();
        for (idx, ctx) in native_contexts.iter().enumerate() {
            context_index_by_ordinal.insert(ctx.ordinal.0, NativeContextId(idx as u32));
        }

        let fixed_owner: Vec<u32> = (0..self.node_count)
            .map(|ordinal| {
                self.infer_fixed_native_context_for_object(
                    ordinal,
                    &context_index_by_ordinal,
                    edge_targets,
                )
            })
            .collect();

        let mut reach_owner = vec![NO_NATIVE_CONTEXT_ID; self.node_count];
        let mut queue = Vec::with_capacity(self.node_count);
        for (ordinal, owner) in fixed_owner.iter().enumerate() {
            if native_context_id_from_raw(*owner).is_some() {
                queue.push(ordinal);
            }
        }

        let mut queue_index = 0;
        while queue_index < queue.len() {
            let ordinal = queue[queue_index];
            queue_index += 1;

            let current = if native_context_id_from_raw(fixed_owner[ordinal]).is_some() {
                fixed_owner[ordinal]
            } else {
                reach_owner[ordinal]
            };
            if current == NO_NATIVE_CONTEXT_ID {
                continue;
            }

            let (first_edge, last_edge) = self.node_edge_range(ordinal);
            let mut ei = first_edge;
            while ei < last_edge {
                let edge_type = self.edge_type_raw(ei);
                if edge_type == self.edge_shortcut_type {
                    ei += 1;
                    continue;
                }
                let child_ordinal = self.edge_to_node_ordinal(ei);
                if child_ordinal == ordinal
                    || native_context_id_from_raw(fixed_owner[child_ordinal]).is_some()
                {
                    ei += 1;
                    continue;
                }
                let merged = Self::merge_native_context_owner(reach_owner[child_ordinal], current);
                if merged != reach_owner[child_ordinal] {
                    reach_owner[child_ordinal] = merged;
                    queue.push(child_ordinal);
                }
                ei += 1;
            }
        }

        let mut node_native_context_buckets =
            vec![NativeContextBucket::Unattributed; self.node_count];
        for ordinal in 0..self.node_count {
            let owner = if native_context_id_from_raw(fixed_owner[ordinal]).is_some() {
                fixed_owner[ordinal]
            } else {
                reach_owner[ordinal]
            };
            node_native_context_buckets[ordinal] = match owner {
                SHARED_NATIVE_CONTEXT_ID => NativeContextBucket::Shared,
                NO_NATIVE_CONTEXT_ID => NativeContextBucket::Unattributed,
                id => NativeContextBucket::Context(NativeContextId(id)),
            };
        }
        node_native_context_buckets
    }

    fn compute_native_context_attributable_sizes_output(
        &self,
        native_context_count: usize,
        node_native_context_buckets: &[NativeContextBucket],
    ) -> (Vec<u64>, u64, u64) {
        let mut native_context_sizes = vec![0u64; native_context_count];
        let mut shared_size = 0u64;
        let mut unattributed_size = 0u64;

        for ordinal in 0..self.node_count {
            let size = self.node_self_size(NodeOrdinal(ordinal)) as u64;
            match node_native_context_buckets[ordinal] {
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

        (native_context_sizes, shared_size, unattributed_size)
    }

    fn infer_fixed_native_context_for_object(
        &self,
        ordinal: usize,
        context_index_by_ordinal: &FxHashMap<usize, NativeContextId>,
        edge_targets: &InitEdgeTargets,
    ) -> u32 {
        if let Some(id) = context_index_by_ordinal.get(&ordinal) {
            return id.0;
        }

        let Some(map_ordinal) = edge_target_ordinal(edge_targets.map[ordinal]) else {
            return NO_NATIVE_CONTEXT_ID;
        };

        // Mirror V8 NativeContextInferrer::Infer: object map -> map's map
        // carries the native context used for attribution.
        let Some(meta_map_ordinal) = edge_target_ordinal(edge_targets.map[map_ordinal.0]) else {
            return NO_NATIVE_CONTEXT_ID;
        };
        let Some(native_context_ordinal) =
            edge_target_ordinal(edge_targets.native_context[meta_map_ordinal.0])
        else {
            return NO_NATIVE_CONTEXT_ID;
        };
        context_index_by_ordinal
            .get(&native_context_ordinal.0)
            .map(|id| id.0)
            .unwrap_or(NO_NATIVE_CONTEXT_ID)
    }

    fn find_js_globals_output(&self) -> (Vec<usize>, Vec<usize>) {
        let mut js_global_objects = Vec::new();
        let mut js_global_proxies = Vec::new();
        for ordinal in 0..self.node_count {
            let name = self.node_raw_name(NodeOrdinal(ordinal));
            if !name.contains("[JSGlobal") {
                continue;
            }
            if Self::has_global_tag(name, "[JSGlobalObject]") {
                js_global_objects.push(ordinal);
            } else if Self::has_global_tag(name, "[JSGlobalProxy]") {
                js_global_proxies.push(ordinal);
            }
        }
        (js_global_objects, js_global_proxies)
    }

    fn native_context_global_fields() -> FxHashSet<String> {
        let mut fields: FxHashSet<String> = Self::KNOWN_GLOBAL_FIELDS
            .iter()
            .map(|&s| s.to_string())
            .collect();
        for &handler in Self::KNOWN_GLOBAL_EVENT_HANDLERS {
            fields.insert(handler.to_string());
            fields.insert(format!("get {handler}"));
            fields.insert(format!("set {handler}"));
        }
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
            fields.insert(format!("get {attr}"));
            fields.insert(format!("set {attr}"));
        }
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
            fields.insert(format!("get {attr}"));
        }
        fields
    }

    fn compute_native_context_vars(
        &self,
        native_contexts: &[NativeContextData],
        native_context_global_fields: &FxHashSet<String>,
        edge_targets: &InitEdgeTargets,
    ) -> FxHashMap<NodeOrdinal, String> {
        let mut native_context_vars = FxHashMap::default();
        for ctx in native_contexts {
            let mut vars = self.native_context_global_unique_fields_from_set(
                ctx.ordinal,
                native_context_global_fields,
            );
            let script_vars = edge_target_ordinal(edge_targets.script_context_table[ctx.ordinal.0])
                .map(|table| self.script_context_table_vars(table))
                .unwrap_or_default();
            for v in script_vars {
                if !vars.contains(&v) {
                    vars.push(v);
                }
            }
            vars.sort();
            native_context_vars.insert(ctx.ordinal, vars.join(", "));
        }
        native_context_vars
    }

    fn native_context_global_unique_fields_from_set(
        &self,
        ord: NodeOrdinal,
        native_context_global_fields: &FxHashSet<String>,
    ) -> Vec<String> {
        let Some(global) = self.find_edge_target(ord, "global_object") else {
            return Vec::new();
        };
        let mut unique: Vec<String> = self
            .iter_edges(global)
            .filter_map(|(edge_idx, _)| {
                let edge_type = self.edge_type_raw(edge_idx.0);
                if edge_type == self.edge_element_type || edge_type == self.edge_hidden_type {
                    return None;
                }
                let name_idx = self.edge_name_or_index(edge_idx.0) as usize;
                let name = &self.strings[name_idx];
                if native_context_global_fields.contains(name.as_str()) {
                    None
                } else {
                    Some(name.clone())
                }
            })
            .collect();
        unique.sort();
        unique
    }

    fn build_init_edge_targets(&self) -> InitEdgeTargets {
        let mut targets = InitEdgeTargets::new(self.node_count);
        for ordinal in 0..self.node_count {
            let (first, last) = self.node_edge_range(ordinal);
            let mut ei = first;
            while ei < last {
                let edge_type = self.edge_type_raw(ei);
                if edge_type == self.edge_element_type || edge_type == self.edge_hidden_type {
                    ei += 1;
                    continue;
                }

                let name_idx = self.edge_name_or_index(ei) as usize;
                let child_ordinal = self.edge_to_node_ordinal(ei) as u32;
                match self.strings[name_idx].as_str() {
                    "global_object" if targets.global_object[ordinal] == NO_EDGE_TARGET => {
                        targets.global_object[ordinal] = child_ordinal;
                    }
                    "global_proxy_object"
                        if targets.global_proxy_object[ordinal] == NO_EDGE_TARGET =>
                    {
                        targets.global_proxy_object[ordinal] = child_ordinal;
                    }
                    "native_context"
                        if edge_type == self.edge_internal_type
                            && targets.native_context[ordinal] == NO_EDGE_TARGET =>
                    {
                        targets.native_context[ordinal] = child_ordinal;
                    }
                    "map"
                        if edge_type == self.edge_internal_type
                            && targets.map[ordinal] == NO_EDGE_TARGET =>
                    {
                        targets.map[ordinal] = child_ordinal;
                    }
                    "script_context_table"
                        if targets.script_context_table[ordinal] == NO_EDGE_TARGET =>
                    {
                        targets.script_context_table[ordinal] = child_ordinal;
                    }
                    _ => {}
                }
                ei += 1;
            }
        }
        targets
    }

    fn build_retainers_output(&self) -> RetainerIndex {
        let edge_count = self.edge_count;

        let mut first_retainer_index = vec![0u32; self.node_count + 1];
        for edge in &self.edges {
            first_retainer_index[edge.to_node_ordinal as usize] += 1;
        }

        let mut first_unused = 0u32;
        for i in 0..self.node_count {
            let count = first_retainer_index[i];
            first_retainer_index[i] = first_unused;
            first_unused += count;
        }
        first_retainer_index[self.node_count] = edge_count as u32;

        let mut next_retainer_index = first_retainer_index.clone();
        let mut retaining_nodes = vec![0u32; edge_count];
        let mut retaining_edges = vec![0u32; edge_count];
        for src_ordinal in 0..self.node_count {
            let (first_edge, next_first) = self.node_edge_range(src_ordinal);
            let mut edge_index = first_edge;
            while edge_index < next_first {
                let to_ordinal = self.edge_to_node_ordinal(edge_index);
                next_retainer_index[to_ordinal + 1] -= 1;
                let slot = next_retainer_index[to_ordinal + 1] as usize;
                retaining_nodes[slot] = src_ordinal as u32;
                retaining_edges[slot] = edge_index as u32;
                edge_index += 1;
            }
        }

        RetainerIndex {
            retaining_nodes,
            retaining_edges,
            first_retainer_index,
        }
    }

    pub(super) fn compute_detachedness(&self) -> Vec<Detachedness> {
        let mut detachedness = vec![Detachedness::Unknown; self.node_count];
        if self.node_detachedness_offset == -1 {
            return detachedness;
        }

        let mut visited = vec![0u8; self.node_count];
        let mut attached: Vec<NodeOrdinal> = Vec::new();
        let mut detached: Vec<NodeOrdinal> = Vec::new();

        for ordinal in 0..self.node_count {
            let raw = self.nodes[ordinal].detachedness;
            let det = decode_detachedness(raw);
            detachedness[ordinal] = det;

            if self.node_type_raw(ordinal) != self.node_native_type {
                continue;
            }
            match det {
                Detachedness::Attached => {
                    attached.push(NodeOrdinal(ordinal));
                    visited[ordinal] = 1;
                }
                Detachedness::Detached => {
                    detached.push(NodeOrdinal(ordinal));
                    visited[ordinal] = 1;
                }
                Detachedness::Unknown => {}
            }
        }

        while let Some(ordinal) = attached.pop() {
            let (first_edge, last_edge) = self.node_edge_range(ordinal.0);
            let mut edge_index = first_edge;
            while edge_index < last_edge {
                let edge_type = self.edge_type_raw(edge_index);
                if edge_type == self.edge_weak_type || edge_type == self.edge_invisible_type {
                    edge_index += 1;
                    continue;
                }
                let child_ordinal = self.edge_to_node_ordinal(edge_index);
                if self.node_type_raw(child_ordinal) != self.node_native_type {
                    edge_index += 1;
                    continue;
                }
                if visited[child_ordinal] != 0 {
                    edge_index += 1;
                    continue;
                }
                visited[child_ordinal] = 1;
                detachedness[child_ordinal] = Detachedness::Attached;
                attached.push(NodeOrdinal(child_ordinal));
                edge_index += 1;
            }
        }

        while let Some(ordinal) = detached.pop() {
            let (first_edge, last_edge) = self.node_edge_range(ordinal.0);
            let mut edge_index = first_edge;
            while edge_index < last_edge {
                let edge_type = self.edge_type_raw(edge_index);
                if edge_type == self.edge_weak_type || edge_type == self.edge_invisible_type {
                    edge_index += 1;
                    continue;
                }
                let child_ordinal = self.edge_to_node_ordinal(edge_index);
                if self.node_type_raw(child_ordinal) != self.node_native_type {
                    edge_index += 1;
                    continue;
                }
                if visited[child_ordinal] != 0 {
                    edge_index += 1;
                    continue;
                }
                visited[child_ordinal] = 1;
                detachedness[child_ordinal] = Detachedness::Detached;
                detached.push(NodeOrdinal(child_ordinal));
                edge_index += 1;
            }
        }

        self.mark_nodes_retained_by_detached_nodes_output(&mut detachedness);
        detachedness
    }

    fn mark_nodes_retained_by_detached_nodes_output(&self, detachedness: &mut [Detachedness]) {
        let normal_reachable = self.reachable_from_roots_with_detachedness(detachedness, false);
        let reachable_without_detached =
            self.reachable_from_roots_with_detachedness(detachedness, true);

        for ordinal in 0..self.node_count {
            if normal_reachable[ordinal] && !reachable_without_detached[ordinal] {
                detachedness[ordinal] = Detachedness::Detached;
            }
        }
    }

    fn reachable_from_roots_with_detachedness(
        &self,
        detachedness: &[Detachedness],
        stop_at_detached: bool,
    ) -> Vec<bool> {
        let mut pending_ephemerons = FxHashSet::default();
        let mut reachable = vec![false; self.node_count];
        let mut queue = Vec::with_capacity(self.node_count);
        for root in &self.system_roots {
            assert!(!reachable[root.0], "duplicate system root: {root:?}");
            reachable[root.0] = true;
            queue.push(root.0);
        }

        let mut queue_index = 0;
        while queue_index < queue.len() {
            let ordinal = queue[queue_index];
            queue_index += 1;

            if stop_at_detached && detachedness[ordinal] == Detachedness::Detached {
                continue;
            }

            let (first_edge, last_edge) = self.node_edge_range(ordinal);
            let mut ei = first_edge;
            while ei < last_edge {
                let edge_type = self.edge_type_raw(ei);
                if edge_type == self.edge_weak_type {
                    ei += 1;
                    continue;
                }

                let child_ordinal = self.edge_to_node_ordinal(ei);
                if reachable[child_ordinal] {
                    ei += 1;
                    continue;
                }

                if !self.should_traverse_reachability_edge(ordinal, ei, &mut pending_ephemerons) {
                    ei += 1;
                    continue;
                }

                reachable[child_ordinal] = true;
                queue.push(child_ordinal);
                ei += 1;
            }
        }

        reachable
    }

    fn calculate_dominator_data<F>(
        &self,
        essential_edges: &Bitmap,
        get_retainer_index: F,
    ) -> (RetainerIndex, DominatorData)
    where
        F: FnOnce() -> RetainerIndex,
    {
        let node_count = self.node_count;
        let root_ordinal = self.gc_roots_ordinal;

        let array_len = node_count + 1;
        let mut parent = vec![0u32; array_len];
        let mut ancestor = vec![0u32; array_len];
        let mut vertex = vec![0u32; array_len];
        let mut label = vec![0u32; array_len];
        let mut semi = vec![0u32; array_len];
        let mut bucket_head = vec![0u32; array_len];
        let mut bucket_next = vec![0u32; array_len];
        let mut n: u32 = 0;

        let retainer_index = {
            #[derive(Clone, Copy)]
            struct DfsFrame {
                node: u32,
                next_edge: u32,
                edge_end: u32,
            }

            let do_dfs = |root: u32,
                          semi: &mut Vec<u32>,
                          n: &mut u32,
                          vertex: &mut Vec<u32>,
                          label: &mut Vec<u32>,
                          parent: &mut Vec<u32>,
                          dfs_stack: &mut Vec<DfsFrame>,
                          nodes: &[NodeRecord],
                          essential_edges: &Bitmap,
                          edges: &[EdgeRecord]| {
                let root_ord = root - 1;
                dfs_stack.clear();
                dfs_stack.push(DfsFrame {
                    node: root,
                    next_edge: nodes[root_ord as usize].first_edge,
                    edge_end: nodes[root_ord as usize].first_edge
                        + nodes[root_ord as usize].edge_count,
                });

                while !dfs_stack.is_empty() {
                    let frame_idx = dfs_stack.len() - 1;
                    let v = dfs_stack[frame_idx].node;
                    if semi[v as usize] == 0 {
                        *n += 1;
                        semi[v as usize] = *n;
                        vertex[*n as usize] = v;
                        label[v as usize] = v;
                    }

                    let mut child = 0u32;
                    while dfs_stack[frame_idx].next_edge < dfs_stack[frame_idx].edge_end {
                        let edge_ordinal = dfs_stack[frame_idx].next_edge as usize;
                        dfs_stack[frame_idx].next_edge += 1;
                        if !essential_edges.get(edge_ordinal) {
                            continue;
                        }
                        let w_ord = edges[edge_ordinal].to_node_ordinal;
                        let w = w_ord + 1;
                        if semi[w as usize] == 0 {
                            parent[w as usize] = v;
                            child = w;
                            break;
                        }
                    }

                    if child == 0 {
                        dfs_stack.pop();
                    } else {
                        let child_ord = child - 1;
                        dfs_stack.push(DfsFrame {
                            node: child,
                            next_edge: nodes[child_ord as usize].first_edge,
                            edge_end: nodes[child_ord as usize].first_edge
                                + nodes[child_ord as usize].edge_count,
                        });
                    }
                }
            };

            let r = (root_ordinal + 1) as u32;
            let mut dfs_stack = Vec::with_capacity(node_count);
            do_dfs(
                r,
                &mut semi,
                &mut n,
                &mut vertex,
                &mut label,
                &mut parent,
                &mut dfs_stack,
                &self.nodes,
                essential_edges,
                &self.edges,
            );

            let retainer_index = get_retainer_index();

            if (n as usize) < node_count {
                for v in 1..=node_count as u32 {
                    if semi[v as usize] == 0 {
                        let v_ord = (v - 1) as usize;
                        if self.has_only_weak_retainers_with_index(
                            NodeOrdinal(v_ord),
                            essential_edges,
                            &retainer_index,
                        ) {
                            parent[v as usize] = r;
                            do_dfs(
                                v,
                                &mut semi,
                                &mut n,
                                &mut vertex,
                                &mut label,
                                &mut parent,
                                &mut dfs_stack,
                                &self.nodes,
                                essential_edges,
                                &self.edges,
                            );
                        }
                    }
                }
            }

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

            retainer_index
        };

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

        let r = (root_ordinal + 1) as u32;
        let mut dom = vec![0u32; array_len];

        for i in (2..=n).rev() {
            let w = vertex[i as usize];
            let w_ord = (w - 1) as usize;

            let mut is_orphan = true;
            let begin_ret = retainer_index.first_retainer_index[w_ord] as usize;
            let end_ret = retainer_index.first_retainer_index[w_ord + 1] as usize;
            for ret_idx in begin_ret..end_ret {
                let ret_edge_ordinal = retainer_index.retaining_edges[ret_idx] as usize;
                if !essential_edges.get(ret_edge_ordinal) {
                    continue;
                }
                is_orphan = false;
                let v_ord = retainer_index.retaining_nodes[ret_idx] as usize;
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
            bucket_next[w as usize] = bucket_head[bkt_idx];
            bucket_head[bkt_idx] = w;
            ancestor[w as usize] = parent[w as usize];

            let pw = parent[w as usize] as usize;
            let mut v = bucket_head[pw];
            bucket_head[pw] = 0;
            while v != 0 {
                let next = bucket_next[v as usize];
                bucket_next[v as usize] = 0;
                let u = evaluate(v, &mut ancestor, &mut label, &semi, &mut compression_stack);
                dom[v as usize] = if semi[u as usize] < semi[v as usize] {
                    u
                } else {
                    parent[w as usize]
                };
                v = next;
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

        let mut immediate_dominators = vec![0u32; node_count];
        let mut retained_sizes = vec![0u64; node_count];
        for ord in 0..node_count {
            immediate_dominators[ord] = dom[ord + 1] - 1;
            retained_sizes[ord] = self.nodes[ord].self_size as u64;
        }

        for i in (2..=n).rev() {
            let ord = (vertex[i as usize] - 1) as usize;
            let dom_ord = immediate_dominators[ord] as usize;
            retained_sizes[dom_ord] += retained_sizes[ord];
        }

        let (dominated_nodes, first_dominated_node_index) =
            self.build_dominated_nodes_output(&immediate_dominators);

        (
            retainer_index,
            DominatorData {
                retained_sizes,
                immediate_dominators,
                dominated_nodes,
                first_dominated_node_index,
            },
        )
    }

    fn has_only_weak_retainers_with_index(
        &self,
        node_ordinal: NodeOrdinal,
        essential_edges: &Bitmap,
        retainer_index: &RetainerIndex,
    ) -> bool {
        let begin = retainer_index.first_retainer_index[node_ordinal.0] as usize;
        let end = retainer_index.first_retainer_index[node_ordinal.0 + 1] as usize;
        for ret_idx in begin..end {
            let edge_ordinal = retainer_index.retaining_edges[ret_idx] as usize;
            if essential_edges.get(edge_ordinal) {
                return false;
            }
        }
        true
    }

    fn build_dominated_nodes_output(&self, immediate_dominators: &[u32]) -> (Vec<u32>, Vec<u32>) {
        let node_count = self.node_count;
        let root_ordinal = self.gc_roots_ordinal;

        let mut index_array = vec![0u32; node_count + 1];
        for ordinal in 0..node_count {
            if ordinal == root_ordinal {
                continue;
            }
            let dom = immediate_dominators[ordinal] as usize;
            index_array[dom] += 1;
        }

        let dominated_count = node_count - 1;
        let mut dominated_nodes = vec![0u32; dominated_count];

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

        for ordinal in 0..node_count {
            if ordinal == root_ordinal {
                continue;
            }
            let dom = immediate_dominators[ordinal] as usize;
            let dom_ref_idx = index_array[dom] as usize;
            dominated_nodes[dom_ref_idx] -= 1;
            let slot = dom_ref_idx + dominated_nodes[dom_ref_idx] as usize;
            dominated_nodes[slot] = ordinal as u32;
        }

        (dominated_nodes, index_array)
    }

    fn calculate_distances_output(&self) -> Vec<Distance> {
        let node_count = self.node_count;
        let mut node_distances = vec![Distance::NONE; node_count];
        let mut nodes_to_visit = vec![0u32; node_count];
        let mut visit_len: usize;
        let mut pending_ephemerons = FxHashSet::default();

        let root_ordinal = self.root_ordinal();
        let gc_roots_ordinal = self.gc_roots_ordinal;

        node_distances[gc_roots_ordinal] = Distance(0);
        nodes_to_visit[0] = gc_roots_ordinal as u32;
        visit_len = 1;
        self.bfs_with_filter_output(
            &mut node_distances,
            &mut nodes_to_visit,
            &mut visit_len,
            &mut pending_ephemerons,
        );

        node_distances[root_ordinal] = Distance(0);
        visit_len = 0;
        let (first, last) = self.node_edge_range(root_ordinal);
        let mut ei = first;
        while ei < last {
            let child_ordinal = self.edge_to_node_ordinal(ei);
            if node_distances[child_ordinal] == Distance::NONE
                && self.root_kinds[child_ordinal] != RootKind::UserRoot
            {
                node_distances[child_ordinal] = Distance(1);
                nodes_to_visit[visit_len] = child_ordinal as u32;
                visit_len += 1;
            }
            ei += 1;
        }
        if visit_len > 0 {
            self.bfs_with_filter_output(
                &mut node_distances,
                &mut nodes_to_visit,
                &mut visit_len,
                &mut pending_ephemerons,
            );
        }
        node_distances
    }

    fn bfs_with_filter_output(
        &self,
        node_distances: &mut [Distance],
        nodes_to_visit: &mut [u32],
        visit_len: &mut usize,
        pending_ephemerons: &mut FxHashSet<String>,
    ) {
        let mut index = 0;
        while index < *visit_len {
            let node_ordinal = nodes_to_visit[index] as usize;
            index += 1;
            let distance = Distance(node_distances[node_ordinal].0 + 1);
            let (first_edge, last_edge) = self.node_edge_range(node_ordinal);
            let mut ei = first_edge;
            while ei < last_edge {
                let edge_type = self.edge_type_raw(ei);
                if edge_type == self.edge_weak_type {
                    ei += 1;
                    continue;
                }
                let child_ordinal = self.edge_to_node_ordinal(ei);
                if node_distances[child_ordinal] != Distance::NONE {
                    ei += 1;
                    continue;
                }

                if !self.should_traverse_reachability_edge(node_ordinal, ei, pending_ephemerons) {
                    ei += 1;
                    continue;
                }

                node_distances[child_ordinal] = distance;
                nodes_to_visit[*visit_len] = child_ordinal as u32;
                *visit_len += 1;
                ei += 1;
            }
        }
    }

    fn compute_name_data(&self) -> NameData {
        let node_count = self.node_count;
        let base_string_count = self.strings.len() as u32;

        let mut class_indices = vec![0u32; self.node_count];
        let mut appended_strings = Vec::new();
        let mut string_table: FxHashMap<String, u32> = FxHashMap::default();

        let get_index =
            |s: &str, strings: &mut Vec<String>, table: &mut FxHashMap<String, u32>| -> u32 {
                if let Some(&idx) = table.get(s) {
                    idx
                } else {
                    let idx = base_string_count + strings.len() as u32;
                    strings.push(s.to_string());
                    table.insert(s.to_string(), idx);
                    idx
                }
            };

        let function_idx = get_index("Function", &mut appended_strings, &mut string_table);
        let regexp_idx = get_index("RegExp", &mut appended_strings, &mut string_table);

        for ordinal in 0..node_count {
            let raw_type = self.node_type_raw(ordinal);
            let raw_name_idx = self.nodes[ordinal].name;

            let class_index =
                if raw_type == self.node_hidden_type || raw_type == self.node_code_type {
                    let name = self.strings[raw_name_idx as usize].clone();
                    let fallback = if raw_type == self.node_hidden_type {
                        "(hidden)"
                    } else {
                        "(code)"
                    };
                    if name.is_empty() {
                        get_index(fallback, &mut appended_strings, &mut string_table)
                    } else if name.starts_with("system / ") {
                        let class_name = match name.match_indices(" / ").nth(1) {
                            Some((pos, _)) => name[..pos].to_string(),
                            None => name,
                        };
                        get_index(&class_name, &mut appended_strings, &mut string_table)
                    } else {
                        get_index(&name, &mut appended_strings, &mut string_table)
                    }
                } else if raw_type == self.node_object_type || raw_type == self.node_native_type {
                    let name = self.strings[raw_name_idx as usize].clone();
                    if name.starts_with('<') {
                        let first_space = name.find(' ');
                        let short_name = if let Some(pos) = first_space {
                            format!("{}>", &name[..pos])
                        } else {
                            name
                        };
                        get_index(&short_name, &mut appended_strings, &mut string_table)
                    } else {
                        raw_name_idx
                    }
                } else if raw_type == self.node_closure_type {
                    function_idx
                } else if raw_type == self.node_regexp_type {
                    regexp_idx
                } else {
                    let type_name = if (raw_type as usize) < self.node_types.len() {
                        self.node_types[raw_type as usize].clone()
                    } else {
                        format!("unknown_{}", raw_type)
                    };
                    get_index(
                        &format!("({})", type_name),
                        &mut appended_strings,
                        &mut string_table,
                    )
                };

            class_indices[ordinal] = class_index;
        }

        self.apply_interface_definitions_to_names(&mut class_indices, &mut appended_strings);

        NameData {
            class_indices,
            appended_strings,
        }
    }

    fn apply_interface_definitions_to_names(
        &self,
        class_indices: &mut [u32],
        appended_strings: &mut Vec<String>,
    ) {
        let edge_prop_type = self.edge_property_type;
        let base_string_count = self.strings.len() as u32;

        struct Candidate {
            name: String,
            properties: Vec<String>,
            count: u32,
        }

        let mut candidates: FxHashMap<String, Candidate> = FxHashMap::default();
        let mut total_object_count = 0u32;

        for ordinal in 0..self.node_count {
            let raw_type = self.node_type_raw(ordinal);
            let raw_name_idx = self.node_name_index(ordinal);
            if raw_type != self.node_object_type || self.strings[raw_name_idx] != "Object" {
                continue;
            }
            total_object_count += 1;

            let mut interface_name = "{".to_string();
            let mut properties: Vec<String> = Vec::new();
            let (first_edge, last_edge) = self.node_edge_range(ordinal);
            let mut ei = first_edge;
            while ei < last_edge {
                let et = self.edge_type_raw(ei);
                if et != edge_prop_type {
                    ei += 1;
                    continue;
                }
                let name_idx = self.edge_name_or_index(ei) as usize;
                let edge_name = self.strings[name_idx].clone();
                if edge_name == "__proto__" {
                    ei += 1;
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
                ei += 1;
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

        struct TrieNode {
            next: FxHashMap<String, usize>,
            match_name: Option<(String, usize, usize)>,
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

            let mut current = 0usize;
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
            if trie_nodes[current].match_name.is_none() {
                trie_nodes[current].match_name =
                    Some((def.name.clone(), sorted_props.len(), def_idx));
            }
        }

        let mut interface_names: FxHashMap<String, u32> = FxHashMap::default();

        for ordinal in 0..self.node_count {
            let raw_type = self.node_type_raw(ordinal);
            let raw_name_idx = self.node_name_index(ordinal);
            if raw_type != self.node_object_type || self.strings[raw_name_idx] != "Object" {
                continue;
            }

            let mut properties: Vec<String> = Vec::new();
            let (first_edge, last_edge) = self.node_edge_range(ordinal);
            let mut ei = first_edge;
            while ei < last_edge {
                let et = self.edge_type_raw(ei);
                if et == edge_prop_type {
                    let name_idx = self.edge_name_or_index(ei) as usize;
                    properties.push(self.strings[name_idx].clone());
                }
                ei += 1;
            }
            properties.sort();

            let mut states: Vec<usize> = vec![0];
            let mut best: Option<(String, usize, usize)> = trie_nodes[0].match_name.clone();

            for prop in &properties {
                let current_states: Vec<usize> = states.clone();
                for &state in &current_states {
                    if let Some(ref greatest) = trie_nodes[state].greatest_next {
                        if prop >= greatest {
                            states.retain(|&s| s != state);
                        }
                    } else {
                        states.retain(|&s| s != state);
                    }

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

            if let Some((ref match_name, _, _)) = best {
                let class_idx = if let Some(&idx) = interface_names.get(match_name) {
                    idx
                } else {
                    let idx = base_string_count + appended_strings.len() as u32;
                    appended_strings.push(match_name.clone());
                    interface_names.insert(match_name.clone(), idx);
                    idx
                };
                class_indices[ordinal] = class_idx;
            }
        }
    }

    fn compute_retained_by_context_output(
        &self,
        node_distances: &[Distance],
    ) -> (RetainedByContext, Vec<ContextRetention>) {
        let reachability =
            self.compute_retained_by_context_reachability_for_distances(node_distances);

        let mut retained_by_context_size = 0u64;
        let mut not_retained_by_context_size = 0u64;

        for ordinal in 0..self.node_count {
            let size = self.nodes[ordinal].self_size as u64;
            match reachability.context_retention[ordinal] {
                ContextRetention::Retained => {
                    retained_by_context_size += size;
                }
                ContextRetention::NotRetained => {
                    not_retained_by_context_size += size;
                }
                ContextRetention::Unreachable => {}
            }
        }

        (
            RetainedByContext {
                context_count: reachability.context_count,
                retained_by_context_size,
                not_retained_by_context_size,
            },
            reachability.context_retention,
        )
    }

    fn compute_retained_by_context_reachability_for_distances(
        &self,
        node_distances: &[Distance],
    ) -> RetainedByContextReachability {
        let mut blocked_contexts = vec![false; self.node_count];
        let mut context_count = 0u32;
        for ordinal in 0..self.node_count {
            if self.is_context_object(NodeOrdinal(ordinal)) {
                blocked_contexts[ordinal] = true;
                context_count += 1;
            }
        }

        let reachable_without_contexts = self.reachable_without_blocked_contexts(&blocked_contexts);
        let mut context_retention = vec![ContextRetention::Unreachable; self.node_count];
        for ordinal in 0..self.node_count {
            context_retention[ordinal] = if node_distances[ordinal].is_unreachable() {
                ContextRetention::Unreachable
            } else if reachable_without_contexts[ordinal] {
                ContextRetention::NotRetained
            } else {
                ContextRetention::Retained
            };
        }

        RetainedByContextReachability {
            context_count,
            context_retention,
        }
    }

    fn merge_native_context_owner(current: u32, incoming: u32) -> u32 {
        if current == SHARED_NATIVE_CONTEXT_ID || incoming == SHARED_NATIVE_CONTEXT_ID {
            SHARED_NATIVE_CONTEXT_ID
        } else if current == NO_NATIVE_CONTEXT_ID {
            incoming
        } else if incoming == NO_NATIVE_CONTEXT_ID || current == incoming {
            current
        } else {
            SHARED_NATIVE_CONTEXT_ID
        }
    }

    fn common_edge_names(&self, ordinals: &[usize]) -> FxHashSet<String> {
        let mut common: Option<FxHashSet<String>> = None;
        for &ord in ordinals {
            let mut fields = FxHashSet::default();
            for (edge_idx, _) in self.iter_edges(NodeOrdinal(ord)) {
                let edge_type = self.edge_type_raw(edge_idx.0);
                if edge_type != self.edge_element_type
                    && edge_type != self.edge_hidden_type
                    && !self.is_invisible_edge(edge_idx)
                {
                    let name_idx = self.edge_name_or_index(edge_idx.0) as usize;
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

    pub(super) fn init_essential_edges(&self) -> Bitmap {
        let mut essential = Bitmap::new(self.edge_count);

        for ordinal in 0..self.node_count {
            let (first_edge, last_edge) = self.node_edge_range(ordinal);
            let mut ei = first_edge;
            while ei < last_edge {
                if self.is_essential_edge(ordinal, ei) {
                    essential.set(ei);
                }
                ei += 1;
            }
        }
        essential
    }

    fn is_essential_edge(&self, source_ordinal: usize, edge_ordinal: usize) -> bool {
        let edge_type = self.edge_type_raw(edge_ordinal);

        // Difference from DevTools:
        // WeakMap ephemeron edges are emitted twice: key→value and table→value.
        // DevTools keeps key→value as essential and drops table→value. We do
        // the opposite so the WeakMap table, not the key, dominates the value.
        // That keeps retained sizes from charging ephemeron values to keys that
        // do not actually own them.
        if edge_type == self.edge_internal_type {
            let edge_name_index = self.edge_name_or_index(edge_ordinal) as usize;
            let edge_name = &self.strings[edge_name_index];
            if maybe_weak_map_ephemeron_edge_name(edge_name)
                && let Some(caps) = weak_map_ephemeron_edge_regex().captures(edge_name)
            {
                if let Some(table_id_str) = caps.get(2) {
                    let node_id = self.nodes[source_ordinal].id;
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

        // Ignore self edges
        if source_ordinal == self.edge_to_node_ordinal(edge_ordinal) {
            return false;
        }

        if source_ordinal != self.gc_roots_ordinal {
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
        if source_ordinal == self.root_ordinal() {
            return false;
        }

        // Difference from DevTools:
        // DevTools also filters non-page→page edges here using the
        // `pageObject` flag populated by `markPageOwnedNodes()`. We omit that
        // branch because our target snapshots do not have user-root/page-owned
        // structure, so the flag would never be meaningfully set.

        true
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
                    let retainer_ordinal = self.retaining_nodes[idx] as usize;
                    let ret_dist = self.node_distances[retainer_ordinal];
                    if !ret_dist.is_reachable() || ret_dist >= min_weak_reachable_dist {
                        continue;
                    }
                    let edge_index = self.retaining_edges[idx] as usize;
                    let edge_type = self.edge_type_raw(edge_index);
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
                let retainer_ordinal = self.retaining_nodes[idx] as usize;
                let ret_dist = self.node_distances[retainer_ordinal];
                if ret_dist.is_reachable() {
                    has_reachable_retainer = true;
                    break;
                }
                let edge_index = self.retaining_edges[idx] as usize;
                let edge_type = self.edge_type_raw(edge_index);
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
            let (first_edge, last_edge) = self.node_edge_range(ordinal);
            let mut ei = first_edge;
            while ei < last_edge {
                let edge_type = self.edge_type_raw(ei);
                if edge_type == self.edge_weak_type && !self.weak_is_reachable {
                    ei += 1;
                    continue;
                }
                if !self.is_structural_reachability_edge(ordinal, ei) {
                    ei += 1;
                    continue;
                }
                let child_ordinal = self.edge_to_node_ordinal(ei);
                if self.node_distances[child_ordinal] == Distance::NONE
                    || distance < self.node_distances[child_ordinal]
                {
                    self.node_distances[child_ordinal] = distance;
                    heap.push(Reverse((distance.0, child_ordinal)));
                }
                ei += 1;
            }
        }
    }

    /// Find the `(GC roots)` node among the root's direct children.
    fn find_gc_roots_ordinal(&self) -> Option<usize> {
        let synthetic_root = NodeOrdinal(self.root_ordinal());
        self.find_child_by_node_name(synthetic_root, "(GC roots)")
            .map(|o| o.0)
    }

    fn should_traverse_reachability_edge(
        &self,
        source_ordinal: usize,
        edge_index: usize,
        pending_ephemerons: &mut rustc_hash::FxHashSet<String>,
    ) -> bool {
        if !self.is_structural_reachability_edge(source_ordinal, edge_index) {
            return false;
        }

        let edge_name_or_index = self.edge_name_or_index(edge_index);
        let edge_type = self.edge_type_raw(edge_index);

        // WeakMap ephemeron filtering
        if edge_type == self.edge_internal_type {
            let edge_name_index = edge_name_or_index as usize;
            if edge_name_index < self.strings.len() {
                let edge_name = &self.strings[edge_name_index];
                if maybe_weak_map_ephemeron_edge_name(edge_name)
                    && let Some(caps) = weak_map_ephemeron_edge_regex().captures(edge_name)
                {
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

    /// Stateless structural edge filter shared by reachability passes.
    /// Returns `false` for edges that should never contribute to traversal
    /// (sloppy_function_map, descriptor-array internals).  Does *not* cover
    /// the stateful WeakMap/ephemeron filter.
    fn is_structural_reachability_edge(&self, source_ordinal: usize, edge_index: usize) -> bool {
        let edge_type = self.edge_type_raw(edge_index);
        let edge_name_or_index = self.edge_name_or_index(edge_index);

        // Filter sloppy_function_map in NativeContext
        if self.is_native_context(NodeOrdinal(source_ordinal)) {
            if edge_type != self.edge_element_type && edge_type != self.edge_hidden_type {
                let edge_name = &self.strings[edge_name_or_index as usize];
                if edge_name == "sloppy_function_map" {
                    return false;
                }
            }
        }

        // Filter descriptor array edges
        let node_type = self.node_type_raw(source_ordinal);
        if node_type == self.node_array_type {
            let node_name = &self.strings[self.node_name_index(source_ordinal)];
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

    pub(super) fn calculate_statistics(&self) -> Statistics {
        let mut size_native = self.extra_native_bytes;
        let mut size_typed_arrays = 0u64;
        let mut size_code = 0u64;
        let mut size_strings = 0u64;
        let mut size_js_arrays = 0u64;
        let mut size_system = 0u64;
        let mut unreachable_count = 0u32;
        let mut unreachable_size = 0u64;
        let retained_by_context = self.retained_by_context_data().retained_by_context;

        for ordinal in 0..self.node_count {
            let node_size = self.nodes[ordinal].self_size as u64;
            let node_type = self.node_type_raw(ordinal);

            if self.node_distances[ordinal].is_unreachable() && node_size > 0 {
                unreachable_count += 1;
                unreachable_size += node_size;
            }

            if node_type == self.node_hidden_type {
                size_system += node_size;
                continue;
            }

            if node_type == self.node_native_type {
                size_native += node_size;
                let name = &self.strings[self.node_name_index(ordinal)];
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
                let name = &self.strings[self.node_name_index(ordinal)];
                if name == "Array" {
                    size_js_arrays += self.calculate_array_size(NodeOrdinal(ordinal));
                }
            }
        }

        let total =
            self.dominator_data().retained_sizes[self.gc_roots_ordinal] + self.extra_native_bytes;

        Statistics {
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
            context_count: retained_by_context.context_count,
            retained_by_context_size: retained_by_context.retained_by_context_size,
            not_retained_by_context_size: retained_by_context.not_retained_by_context_size,
        }
    }

    fn reachable_without_blocked_contexts(&self, blocked_contexts: &[bool]) -> Vec<bool> {
        let mut pending_ephemerons = FxHashSet::default();
        let mut queue = Vec::with_capacity(self.node_count);
        let mut visited = vec![false; self.node_count];
        for root in &self.system_roots {
            if !visited[root.0] {
                visited[root.0] = true;
                queue.push(root.0);
            }
        }

        let mut queue_index = 0;
        while queue_index < queue.len() {
            let ordinal = queue[queue_index];
            queue_index += 1;

            let (first_edge, last_edge) = self.node_edge_range(ordinal);
            let mut ei = first_edge;
            while ei < last_edge {
                let edge_type = self.edge_type_raw(ei);
                if edge_type == self.edge_weak_type {
                    ei += 1;
                    continue;
                }

                let child_ordinal = self.edge_to_node_ordinal(ei);
                if blocked_contexts[child_ordinal] || visited[child_ordinal] {
                    ei += 1;
                    continue;
                }

                if !self.should_traverse_reachability_edge(ordinal, ei, &mut pending_ephemerons) {
                    ei += 1;
                    continue;
                }

                visited[child_ordinal] = true;
                queue.push(child_ordinal);
                ei += 1;
            }
        }

        visited
    }
}
