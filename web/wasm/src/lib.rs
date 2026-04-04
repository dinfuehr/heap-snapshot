use std::io::Cursor;

use serde::Serialize;
use wasm_bindgen::prelude::*;

use heap_snapshot::parser;
use heap_snapshot::retaining_path::{
    RetainerAutoExpandLimits, RetainerPathEdge, plan_gc_root_retainer_paths,
};
use heap_snapshot::snapshot::{HeapSnapshot, RootKind, SnapshotOptions};
use heap_snapshot::types::AggregateInfo;
use rustc_hash::FxHashMap;
use heap_snapshot::types::{NodeId, NodeOrdinal};

// ---------------------------------------------------------------------------
// Serialization types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct JsStatistics {
    node_count: usize,
    total: f64,
    v8heap_total: f64,
    native_total: f64,
    code: f64,
    strings: f64,
    js_arrays: f64,
    typed_arrays: f64,
    system: f64,
    extra_native_bytes: f64,
    unreachable_size: f64,
    unreachable_count: u32,
}

#[derive(Serialize)]
struct JsAggregateEntry {
    key: String,
    name: String,
    count: u32,
    self_size: f64,
    retained_size: f64,
}

#[derive(Serialize)]
struct JsNodeInfo {
    id: u64,
    name: String,
    node_type: String,
    self_size: u32,
    retained_size: f64,
    distance: u32,
    edge_count: u32,
    detachedness: u8,
}

#[derive(Serialize)]
struct JsEdge {
    edge_type: String,
    edge_name: String,
    target: JsNodeInfo,
}

#[derive(Serialize)]
struct JsRetainer {
    edge_type: String,
    edge_name: String,
    source: JsNodeInfo,
}

#[derive(Serialize)]
struct JsContainment {
    system_roots: Vec<JsEdge>,
    gc_roots_children: Vec<JsEdgeWithChildCount>,
}

#[derive(Serialize)]
struct JsEdgeWithChildCount {
    edge_type: String,
    edge_name: String,
    target: JsNodeInfo,
    child_count: u32,
}

#[derive(Serialize)]
struct JsSummaryObject {
    id: u64,
    name: String,
    self_size: u32,
    retained_size: f64,
    detachedness: u8,
}

#[derive(Serialize)]
struct JsSummaryExpanded {
    constructor: String,
    total: usize,
    objects: Vec<JsSummaryObject>,
}

#[derive(Serialize)]
struct JsRetainingPath {
    edge_type: String,
    edge_name: String,
    node: JsNodeInfo,
    children: Vec<JsRetainingPath>,
}

#[derive(Serialize)]
struct JsRetainingPaths {
    target: JsNodeInfo,
    paths: Vec<JsRetainingPath>,
    reached_gc_roots: bool,
    truncated: bool,
}

#[derive(Serialize)]
struct JsNativeContext {
    id: u64,
    label: String,
    detachedness: String,
    self_size: u32,
    retained_size: f64,
    vars: String,
}

#[derive(Serialize)]
struct JsDominator {
    id: u64,
    name: String,
    node_type: String,
    self_size: u32,
    retained_size: f64,
}

#[derive(Serialize)]
struct JsAllocationFrame {
    function_name: String,
    script_name: String,
    line: u32,
    column: u32,
}

#[derive(Serialize)]
struct JsAllocationStack {
    frames: Vec<JsAllocationFrame>,
}

#[derive(Serialize)]
struct JsTimelineInterval {
    timestamp_us: u64,
    count: u32,
    size: u64,
}

// ---------------------------------------------------------------------------
// WASM wrapper
// ---------------------------------------------------------------------------

#[wasm_bindgen]
pub struct WasmHeapSnapshot {
    inner: HeapSnapshot,
    cached_aggregates: Option<FxHashMap<String, AggregateInfo>>,
}

impl WasmHeapSnapshot {
    fn node_info(&self, ordinal: NodeOrdinal) -> JsNodeInfo {
        let snap = &self.inner;
        JsNodeInfo {
            id: snap.node_id(ordinal).0,
            name: snap.node_display_name(ordinal).to_string(),
            node_type: snap.node_type_name(ordinal).to_string(),
            self_size: snap.node_self_size(ordinal),
            retained_size: snap.node_retained_size(ordinal),
            distance: snap.node_distance(ordinal).0,
            edge_count: snap.node_edge_count(ordinal) as u32,
            detachedness: snap.node_detachedness(ordinal),
        }
    }

    fn resolve_ordinal(&self, node_id: f64) -> Result<NodeOrdinal, JsError> {
        let id = node_id as u64;
        self.inner
            .node_for_snapshot_object_id(NodeId(id))
            .ok_or_else(|| JsError::new(&format!("No object found with id @{id}")))
    }
}

fn to_json<T: Serialize>(val: &T) -> String {
    serde_json::to_string(val).unwrap()
}

#[wasm_bindgen]
impl WasmHeapSnapshot {
    #[wasm_bindgen(constructor)]
    pub fn new(data: &[u8]) -> Result<WasmHeapSnapshot, JsError> {
        let cursor = Cursor::new(data);
        let raw = parser::parse_from_reader(cursor)
            .map_err(|e| JsError::new(&format!("Parse error: {e}")))?;
        let inner = HeapSnapshot::new_with_options(
            raw,
            SnapshotOptions {
                weak_is_reachable: false,
            },
        );
        let cached_aggregates = Some(inner.aggregates_with_filter());
        Ok(WasmHeapSnapshot {
            inner,
            cached_aggregates,
        })
    }

    pub fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    pub fn get_statistics(&self) -> String {
        let stats = self.inner.get_statistics();
        to_json(&JsStatistics {
            node_count: self.inner.node_count(),
            total: stats.total,
            v8heap_total: stats.v8heap_total,
            native_total: stats.native_total,
            code: stats.code,
            strings: stats.strings,
            js_arrays: stats.js_arrays,
            typed_arrays: stats.typed_arrays,
            system: stats.system,
            extra_native_bytes: stats.extra_native_bytes,
            unreachable_size: stats.unreachable_size,
            unreachable_count: stats.unreachable_count,
        })
    }

    /// Recompute cached aggregates with the given unreachable filter mode.
    /// 0 = all objects, 1 = all unreachable, 2 = roots only.
    pub fn set_unreachable_mode(&mut self, mode: u32) {
        self.cached_aggregates = Some(match mode {
            1 => self.inner.unreachable_aggregates(),
            2 => self.inner.unreachable_root_aggregates(),
            _ => self.inner.aggregates_with_filter(),
        });
    }

    pub fn get_summary(&self) -> String {
        let aggregates = self.cached_aggregates.as_ref().unwrap();
        let mut entries: Vec<JsAggregateEntry> = aggregates
            .iter()
            .map(|(key, agg)| JsAggregateEntry {
                key: key.clone(),
                name: agg.name.clone(),
                count: agg.count,
                self_size: agg.self_size,
                retained_size: agg.max_ret,
            })
            .collect();
        entries.sort_by(|a, b| b.retained_size.partial_cmp(&a.retained_size).unwrap());
        to_json(&entries)
    }

    pub fn get_summary_objects(
        &self,
        constructor: &str,
        offset: usize,
        limit: usize,
    ) -> Result<String, JsError> {
        let aggregates = self.cached_aggregates.as_ref().unwrap();
        let entry = aggregates
            .get(constructor)
            .ok_or_else(|| JsError::new(&format!("No constructor group \"{constructor}\"")))?;

        let total = entry.node_ordinals.len();
        let start = offset.min(total);
        let end = (start + limit).min(total);

        let objects: Vec<JsSummaryObject> = entry.node_ordinals[start..end]
            .iter()
            .map(|&ord| {
                let snap = &self.inner;
                JsSummaryObject {
                    id: snap.node_id(ord).0,
                    name: snap.node_display_name(ord).to_string(),
                    self_size: snap.node_self_size(ord),
                    retained_size: snap.node_retained_size(ord),
                    detachedness: snap.node_detachedness(ord),
                }
            })
            .collect();

        Ok(to_json(&JsSummaryExpanded {
            constructor: constructor.to_string(),
            total,
            objects,
        }))
    }

    pub fn get_containment(&self) -> String {
        let snap = &self.inner;
        let root = snap.synthetic_root_ordinal();

        let system_roots: Vec<JsEdge> = snap
            .iter_edges(root)
            .filter(|&(_, child_ord)| snap.root_kind(child_ord) == RootKind::SystemRoot)
            .map(|(edge_idx, child_ord)| JsEdge {
                edge_type: snap.edge_type_name(edge_idx).to_string(),
                edge_name: snap.edge_name(edge_idx).to_string(),
                target: self.node_info(child_ord),
            })
            .collect();

        let gc_roots = snap.gc_roots_ordinal();
        let gc_roots_children: Vec<JsEdgeWithChildCount> = snap
            .iter_edges(gc_roots)
            .map(|(edge_idx, child_ord)| JsEdgeWithChildCount {
                edge_type: snap.edge_type_name(edge_idx).to_string(),
                edge_name: snap.edge_name(edge_idx).to_string(),
                child_count: snap.node_edge_count(child_ord) as u32,
                target: self.node_info(child_ord),
            })
            .collect();

        to_json(&JsContainment {
            system_roots,
            gc_roots_children,
        })
    }

    pub fn get_children(
        &self,
        node_id: f64,
        offset: usize,
        limit: usize,
        filter: &str,
    ) -> Result<String, JsError> {
        let ordinal = self.resolve_ordinal(node_id)?;
        let snap = &self.inner;
        let filter_lower = filter.to_lowercase();
        let mut edges: Vec<(usize, NodeOrdinal)> = if filter_lower.is_empty() {
            snap.iter_edges(ordinal).collect()
        } else {
            snap.iter_edges(ordinal)
                .filter(|&(edge_idx, child_ord)| {
                    let edge_name = snap.edge_name(edge_idx).to_lowercase();
                    let node_name = snap.node_display_name(child_ord).to_lowercase();
                    edge_name.contains(&filter_lower) || node_name.contains(&filter_lower)
                })
                .collect()
        };

        // For NativeContext nodes, pin certain fields first.
        if snap.is_native_context(ordinal) {
            const PINNED: &[&str] = &[
                "scope_info",
                "global_object",
                "global_proxy_object",
                "script_context_table",
            ];
            edges.sort_by_key(|&(edge_idx, _)| {
                let name = snap.edge_name(edge_idx);
                if let Some(pos) = PINNED.iter().position(|&p| p == name) {
                    (0, pos)
                } else {
                    (1, 0)
                }
            });
        }

        let total = edges.len();
        let start = offset.min(total);
        let end = (start + limit).min(total);

        #[derive(Serialize)]
        struct JsChildren {
            total: usize,
            edges: Vec<JsEdge>,
        }

        let result = JsChildren {
            total,
            edges: edges[start..end]
                .iter()
                .map(|&(edge_idx, child_ord)| JsEdge {
                    edge_type: snap.edge_type_name(edge_idx).to_string(),
                    edge_name: snap.edge_name(edge_idx).to_string(),
                    target: self.node_info(child_ord),
                })
                .collect(),
        };
        Ok(to_json(&result))
    }

    pub fn get_retainers(
        &self,
        node_id: f64,
        offset: usize,
        limit: usize,
        filter: &str,
    ) -> Result<String, JsError> {
        let ordinal = self.resolve_ordinal(node_id)?;
        let snap = &self.inner;
        let filter_lower = filter.to_lowercase();
        let retainers: Vec<(usize, NodeOrdinal)> = if filter_lower.is_empty() {
            snap.get_retainers(ordinal)
        } else {
            snap.get_retainers(ordinal)
                .into_iter()
                .filter(|&(edge_idx, ret_ord)| {
                    let edge_name = snap.edge_name(edge_idx).to_lowercase();
                    let node_name = snap.node_display_name(ret_ord).to_lowercase();
                    edge_name.contains(&filter_lower) || node_name.contains(&filter_lower)
                })
                .collect()
        };
        let total = retainers.len();
        let start = offset.min(total);
        let end = (start + limit).min(total);

        #[derive(Serialize)]
        struct JsRetainers {
            total: usize,
            retainers: Vec<JsRetainer>,
        }

        let result = JsRetainers {
            total,
            retainers: retainers[start..end]
                .iter()
                .map(|&(edge_idx, ret_ord)| JsRetainer {
                    edge_type: snap.edge_type_name(edge_idx).to_string(),
                    edge_name: snap.edge_name(edge_idx).to_string(),
                    source: self.node_info(ret_ord),
                })
                .collect(),
        };
        Ok(to_json(&result))
    }

    pub fn get_retaining_paths(
        &self,
        node_id: f64,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<String, JsError> {
        let ordinal = self.resolve_ordinal(node_id)?;

        let plan = plan_gc_root_retainer_paths(
            &self.inner,
            ordinal,
            RetainerAutoExpandLimits {
                max_depth,
                max_nodes,
            },
        );

        fn convert_edges(
            snap: &HeapSnapshot,
            wasm: &WasmHeapSnapshot,
            edges: &[RetainerPathEdge],
        ) -> Vec<JsRetainingPath> {
            edges
                .iter()
                .map(|pe| JsRetainingPath {
                    edge_type: snap.edge_type_name(pe.edge_idx).to_string(),
                    edge_name: snap.edge_name(pe.edge_idx).to_string(),
                    node: wasm.node_info(pe.retainer),
                    children: convert_edges(snap, wasm, &pe.children),
                })
                .collect()
        }

        let result = JsRetainingPaths {
            target: self.node_info(ordinal),
            paths: convert_edges(&self.inner, self, &plan.tree),
            reached_gc_roots: plan.reached_gc_roots,
            truncated: plan.truncated,
        };
        Ok(to_json(&result))
    }

    pub fn get_native_contexts(&self) -> String {
        let snap = &self.inner;
        let contexts: Vec<JsNativeContext> = snap
            .native_contexts()
            .iter()
            .map(|&ord| {
                let ord = NodeOrdinal(ord);
                JsNativeContext {
                    id: snap.node_id(ord).0,
                    label: snap.native_context_label(ord),
                    detachedness: match snap.native_context_detachedness(ord) {
                        1 => "attached".to_string(),
                        2 => "detached".to_string(),
                        _ => "unknown".to_string(),
                    },
                    self_size: snap.node_self_size(ord),
                    retained_size: snap.node_retained_size(ord),
                    vars: snap.native_context_vars(ord).to_string(),
                }
            })
            .collect();
        to_json(&contexts)
    }

    pub fn get_dominators_of(&self, node_id: f64) -> Result<String, JsError> {
        let ordinal = self.resolve_ordinal(node_id)?;
        let snap = &self.inner;

        let mut chain = Vec::new();
        let mut current = ordinal;
        loop {
            let dom = snap.dominator_of(current);
            if dom == current {
                break;
            }
            chain.push(JsDominator {
                id: snap.node_id(dom).0,
                name: snap.node_display_name(dom).to_string(),
                node_type: snap.node_type_name(dom).to_string(),
                self_size: snap.node_self_size(dom),
                retained_size: snap.node_retained_size(dom),
            });
            current = dom;
        }
        Ok(to_json(&chain))
    }

    pub fn get_dominated_children(
        &self,
        node_id: f64,
        offset: usize,
        limit: usize,
    ) -> Result<String, JsError> {
        let ordinal = self.resolve_ordinal(node_id)?;
        let snap = &self.inner;
        let mut children: Vec<NodeOrdinal> = snap.get_dominated_children(ordinal);
        children.sort_by(|a, b| {
            snap.node_retained_size(*b)
                .partial_cmp(&snap.node_retained_size(*a))
                .unwrap()
        });

        let total = children.len();
        let start = offset.min(total);
        let end = (start + limit).min(total);

        #[derive(Serialize)]
        struct JsDominatedChildren {
            total: usize,
            children: Vec<JsNodeInfo>,
        }

        let result = JsDominatedChildren {
            total,
            children: children[start..end]
                .iter()
                .map(|&ord| self.node_info(ord))
                .collect(),
        };
        Ok(to_json(&result))
    }

    pub fn get_dominator_tree_root(&self) -> String {
        let snap = &self.inner;
        let root = snap.gc_roots_ordinal();
        to_json(&self.node_info(root))
    }

    pub fn get_node_info(&self, node_id: f64) -> Result<String, JsError> {
        let ordinal = self.resolve_ordinal(node_id)?;
        Ok(to_json(&self.node_info(ordinal)))
    }

    pub fn get_reachable_size(&self, node_id: f64) -> Result<String, JsError> {
        let ordinal = self.resolve_ordinal(node_id)?;
        let snap = &self.inner;
        let info = snap.reachable_size(&[ordinal]);

        #[derive(Serialize)]
        struct JsReachableSize {
            size: f64,
            native_contexts: Vec<JsNativeContext>,
        }

        let native_contexts = info
            .native_contexts
            .iter()
            .map(|&ctx_ord| JsNativeContext {
                id: snap.node_id(ctx_ord).0,
                label: snap.native_context_label(ctx_ord),
                detachedness: match snap.native_context_detachedness(ctx_ord) {
                    1 => "attached".to_string(),
                    2 => "detached".to_string(),
                    _ => "unknown".to_string(),
                },
                self_size: snap.node_self_size(ctx_ord),
                retained_size: snap.node_retained_size(ctx_ord),
                vars: snap.native_context_vars(ctx_ord).to_string(),
            })
            .collect();

        Ok(to_json(&JsReachableSize {
            size: info.size,
            native_contexts,
        }))
    }

    pub fn get_children_ids(&self, node_id: f64) -> Result<String, JsError> {
        let ordinal = self.resolve_ordinal(node_id)?;
        let ids: Vec<u64> = self
            .inner
            .iter_edges(ordinal)
            .map(|(_, child_ord)| self.inner.node_id(child_ord).0)
            .collect();
        Ok(to_json(&ids))
    }

    pub fn get_constructor_for_node(&self, node_id: f64) -> Result<String, JsError> {
        let ordinal = self.resolve_ordinal(node_id)?;
        let aggregates = self.inner.aggregates_with_filter();
        for (key, agg) in &aggregates {
            if agg.node_ordinals.contains(&ordinal) {
                return Ok(key.clone());
            }
        }
        Err(JsError::new(&format!(
            "Node @{} not found in any aggregate",
            node_id as u64
        )))
    }

    pub fn has_allocation_data(&self) -> bool {
        self.inner.has_allocation_data()
    }

    pub fn get_timeline(&self) -> String {
        let intervals: Vec<JsTimelineInterval> = self
            .inner
            .get_timeline()
            .iter()
            .map(|i| JsTimelineInterval {
                timestamp_us: i.timestamp_us,
                count: i.count,
                size: i.size,
            })
            .collect();
        to_json(&intervals)
    }

    pub fn get_summary_for_interval(&self, interval_index: usize) -> Result<String, JsError> {
        let intervals = self.inner.get_timeline();
        let interval = intervals.get(interval_index).ok_or_else(|| {
            JsError::new(&format!(
                "Invalid interval index {interval_index}, have {} intervals",
                intervals.len()
            ))
        })?;
        let aggregates =
            self.inner
                .aggregates_for_id_range(interval.id_from, interval.id_to);
        let mut entries: Vec<JsAggregateEntry> = aggregates
            .iter()
            .map(|(key, agg)| JsAggregateEntry {
                key: key.clone(),
                name: agg.name.clone(),
                count: agg.count,
                self_size: agg.self_size,
                retained_size: agg.max_ret,
            })
            .collect();
        entries.sort_by(|a, b| b.retained_size.partial_cmp(&a.retained_size).unwrap());
        Ok(to_json(&entries))
    }

    pub fn get_allocation_stack(&self, node_id: f64) -> Result<String, JsError> {
        let ordinal = self.resolve_ordinal(node_id)?;
        match self.inner.get_allocation_stack(ordinal) {
            Some(stack) => {
                let result = JsAllocationStack {
                    frames: stack
                        .iter()
                        .map(|f| JsAllocationFrame {
                            function_name: f.function_name.clone(),
                            script_name: f.script_name.clone(),
                            line: f.line,
                            column: f.column,
                        })
                        .collect(),
                };
                Ok(to_json(&result))
            }
            None => Ok("null".to_string()),
        }
    }
}
