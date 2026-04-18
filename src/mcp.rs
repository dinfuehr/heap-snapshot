use rustc_hash::FxHashMap;
use std::fs::File;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::Mutex;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::schemars;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, tool, tool_handler, tool_router};

use crate::diff;
use crate::parser;
use crate::print::closure_leaks;
use crate::print::diff::{format_signed_count, format_signed_size};
use crate::print::format_size;
use crate::retaining_path::{
    RetainerAutoExpandLimits, RetainerPathEdge, plan_gc_root_retainer_paths,
};
use crate::snapshot::{Detachedness, HeapSnapshot, RootKind, SnapshotOptions};
use crate::types::{AggregateMap, NodeId, NodeOrdinal};
use crate::utils::truncate_str;

// ---------------------------------------------------------------------------
// Parameter types
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct LoadSnapshotParams {
    /// Path to the .heapsnapshot file
    path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CloseSnapshotParams {
    /// Snapshot ID returned by load_snapshot
    snapshot_id: u32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ShowParams {
    /// Snapshot ID returned by load_snapshot
    snapshot_id: u32,
    /// Object ID in the form @12345
    object_id: String,
    /// How many levels of children to expand (default: 1)
    depth: Option<usize>,
    /// Number of children to skip at each level (default: 0)
    offset: Option<usize>,
    /// Maximum number of children to show at each level (default: 100)
    limit: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ShowRetainersParams {
    /// Snapshot ID returned by load_snapshot
    snapshot_id: u32,
    /// Object ID in the form @12345
    object_id: String,
    /// How many levels of retainers to expand (default: 1)
    depth: Option<usize>,
    /// Number of retainers to skip at each level (default: 0)
    offset: Option<usize>,
    /// Maximum number of retainers to show at each level (default: 100)
    limit: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetNativeContextsParams {
    /// Snapshot ID returned by load_snapshot
    snapshot_id: u32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetReachableSizeParams {
    /// Snapshot ID returned by load_snapshot
    snapshot_id: u32,
    /// Object ID in the form @12345
    object_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetRetainingPathsParams {
    /// Snapshot ID returned by load_snapshot
    snapshot_id: u32,
    /// Object ID in the form @12345
    object_id: String,
    /// Maximum depth to search (default: 50)
    max_depth: Option<usize>,
    /// Maximum number of nodes to explore (default: 200)
    max_nodes: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetStatisticsParams {
    /// Snapshot ID returned by load_snapshot
    snapshot_id: u32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetSummaryParams {
    /// Snapshot ID returned by load_snapshot
    snapshot_id: u32,
    /// Constructor name to expand, showing individual objects in that group
    class_name: Option<String>,
    /// Number of objects to skip when expanding a constructor (default: 0)
    offset: Option<usize>,
    /// Maximum number of objects to return when expanding a constructor (default: 20)
    limit: Option<usize>,
    /// Filter objects: attached, detached, unreachable, unreachable-roots, detached-dom, console, event-handlers
    filter: Option<String>,
    /// Show only objects allocated in a specific timeline interval (0-based index)
    filter_interval: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetContainmentParams {
    /// Snapshot ID returned by load_snapshot
    snapshot_id: u32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetDominatorsOfParams {
    /// Snapshot ID returned by load_snapshot
    snapshot_id: u32,
    /// Object ID in the form @12345
    object_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CompareSnapshotsParams {
    /// Snapshot ID of the main (newer) snapshot
    snapshot_id: u32,
    /// Snapshot ID of the baseline (older) snapshot to compare against
    baseline_id: u32,
    /// Constructor name to filter results to a single class
    class_name: Option<String>,
    /// Number of entries to skip (default: 0)
    offset: Option<usize>,
    /// Maximum number of entries to return (default: 20)
    limit: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetDuplicateStringsParams {
    /// Snapshot ID returned by load_snapshot
    snapshot_id: u32,
    /// Number of entries to skip (default: 0)
    offset: Option<usize>,
    /// Maximum number of entries to return (default: 100)
    limit: Option<usize>,
    /// Include the object ids of each duplicate group (default: false)
    show_object_ids: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetClosureLeaksParams {
    /// Snapshot ID returned by load_snapshot
    snapshot_id: u32,
    /// Include contexts where analysis is incomplete (default: false)
    show_incomplete: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetTimelineParams {
    /// Snapshot ID returned by load_snapshot
    snapshot_id: u32,
}

// ---------------------------------------------------------------------------
// Server state
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct McpServer {
    snapshots: Arc<Mutex<FxHashMap<u32, Arc<HeapSnapshot>>>>,
    next_id: Arc<AtomicU32>,
    tool_router: ToolRouter<McpServer>,
}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

#[tool_router]
impl McpServer {
    fn new() -> Self {
        Self {
            snapshots: Arc::new(Mutex::new(FxHashMap::default())),
            next_id: Arc::new(AtomicU32::new(1)),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Load a .heapsnapshot file and return a snapshot_id for use with other tools"
    )]
    async fn load_snapshot(
        &self,
        Parameters(params): Parameters<LoadSnapshotParams>,
    ) -> Result<CallToolResult, McpError> {
        let path = params.path;

        let (snapshot, node_count, path) = tokio::task::spawn_blocking(move || {
            let file = File::open(&path).map_err(|e| {
                McpError::internal_error(format!("Failed to open {path}: {e}"), None)
            })?;

            let raw = parser::parse(file).map_err(|e| {
                McpError::internal_error(format!("Failed to parse {path}: {e}"), None)
            })?;

            let snapshot = HeapSnapshot::new_with_options(
                raw,
                SnapshotOptions {
                    weak_is_reachable: false,
                },
            );

            let node_count = snapshot.node_count();
            Ok::<_, McpError>((snapshot, node_count, path))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task failed: {e}"), None))??;

        let snapshot_id = self.next_id.fetch_add(1, Ordering::Relaxed);

        self.snapshots
            .lock()
            .await
            .insert(snapshot_id, Arc::new(snapshot));

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Loaded snapshot from {path} with {node_count} nodes. snapshot_id: {snapshot_id}"
        ))]))
    }

    #[tool(description = "Close a previously loaded snapshot, freeing its memory")]
    async fn close_snapshot(
        &self,
        Parameters(params): Parameters<CloseSnapshotParams>,
    ) -> Result<CallToolResult, McpError> {
        let removed = self
            .snapshots
            .lock()
            .await
            .remove(&params.snapshot_id)
            .is_some();
        let snapshot_id = params.snapshot_id;

        if removed {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "Closed snapshot {snapshot_id}"
            ))]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "No snapshot found with id {snapshot_id}"
            ))]))
        }
    }

    #[tool(
        description = "Show an object and its outgoing references (edges). Use depth to auto-expand children recursively. The object_id should be in the form @12345."
    )]
    async fn show(
        &self,
        Parameters(params): Parameters<ShowParams>,
    ) -> Result<CallToolResult, McpError> {
        let object_id_str = params
            .object_id
            .strip_prefix('@')
            .unwrap_or(&params.object_id);
        let object_id: u64 = object_id_str.parse().map_err(|_| {
            McpError::invalid_params(
                format!(
                    "Invalid object id: {}. Expected format: @12345",
                    params.object_id
                ),
                None,
            )
        })?;

        let snapshot = {
            let snapshots = self.snapshots.lock().await;
            Arc::clone(snapshots.get(&params.snapshot_id).ok_or_else(|| {
                McpError::invalid_params(
                    format!("No snapshot found with id {}", params.snapshot_id),
                    None,
                )
            })?)
        };

        let ordinal = snapshot
            .node_for_snapshot_object_id(NodeId(object_id))
            .ok_or_else(|| {
                McpError::invalid_params(format!("No object found with id @{object_id}"), None)
            })?;

        let max_depth = params.depth.unwrap_or(1);
        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(100);

        tokio::task::spawn_blocking(move || {
            let mut lines = Vec::new();
            lines.push(format!(
                "Object @{object_id}: {} (type: {}, self_size: {})",
                snapshot.node_display_name(ordinal),
                snapshot.node_type_name(ordinal),
                snapshot.node_self_size(ordinal),
            ));

            if let Some(stack) = snapshot.get_allocation_stack(ordinal) {
                lines.push("  Allocated at:".to_string());
                for frame in &stack {
                    lines.push(format!(
                        "    {}",
                        HeapSnapshot::format_allocation_frame(frame),
                    ));
                }
            }

            if let Some(source) = snapshot.shared_function_info_source(ordinal) {
                lines.push("  Source:".to_string());
                for line in source.lines() {
                    lines.push(format!("    {line}"));
                }
            }

            fn show_edges(
                snap: &HeapSnapshot,
                ordinal: NodeOrdinal,
                depth: usize,
                max_depth: usize,
                offset: usize,
                limit: usize,
                lines: &mut Vec<String>,
            ) {
                let indent = "  ".repeat(depth);
                let edges: Vec<_> = snap.iter_edges(ordinal).collect();
                let total = edges.len();
                let start = offset.min(total);
                let end = (start + limit).min(total);

                for &(edge_idx, child_ord) in &edges[start..end] {
                    let edge_type = snap.edge_type_name(edge_idx);
                    let edge_name = snap.edge_name(edge_idx);
                    let child_id = snap.node_id(child_ord);
                    let child_name = snap.node_display_name(child_ord);
                    let child_type = snap.node_type_name(child_ord);
                    let child_size = snap.node_self_size(child_ord);

                    lines.push(format!(
                        "{indent}--[{edge_type} \"{edge_name}\"]--> @{} {child_name} (type: {child_type}, self_size: {child_size})",
                        child_id.0
                    ));

                    if depth < max_depth {
                        show_edges(snap, child_ord, depth + 1, max_depth, 0, limit, lines);
                    }
                }

                if end < total {
                    lines.push(format!(
                        "{indent}({}-{} of {total} children shown)",
                        start + 1,
                        end
                    ));
                }
            }

            show_edges(&snapshot, ordinal, 1, max_depth, offset, limit, &mut lines);

            Ok(CallToolResult::success(vec![Content::text(
                lines.join("\n"),
            )]))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task failed: {e}"), None))?
    }

    #[tool(
        description = "Show an object and its raw incoming references (retainers). For understanding why an object is kept alive, prefer get_retaining_paths which automatically finds complete paths to GC roots. Use this tool only when you need to browse individual retainers manually. The object_id should be in the form @12345."
    )]
    async fn show_retainers(
        &self,
        Parameters(params): Parameters<ShowRetainersParams>,
    ) -> Result<CallToolResult, McpError> {
        let object_id_str = params
            .object_id
            .strip_prefix('@')
            .unwrap_or(&params.object_id);
        let object_id: u64 = object_id_str.parse().map_err(|_| {
            McpError::invalid_params(
                format!(
                    "Invalid object id: {}. Expected format: @12345",
                    params.object_id
                ),
                None,
            )
        })?;

        let snapshot = {
            let snapshots = self.snapshots.lock().await;
            Arc::clone(snapshots.get(&params.snapshot_id).ok_or_else(|| {
                McpError::invalid_params(
                    format!("No snapshot found with id {}", params.snapshot_id),
                    None,
                )
            })?)
        };

        let ordinal = snapshot
            .node_for_snapshot_object_id(NodeId(object_id))
            .ok_or_else(|| {
                McpError::invalid_params(format!("No object found with id @{object_id}"), None)
            })?;

        let max_depth = params.depth.unwrap_or(1);
        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(100);

        tokio::task::spawn_blocking(move || {
            let mut lines = Vec::new();
            lines.push(format!(
                "Object @{object_id}: {} (type: {}, self_size: {}, retained_size: {:.0})",
                snapshot.node_display_name(ordinal),
                snapshot.node_type_name(ordinal),
                snapshot.node_self_size(ordinal),
                snapshot.node_retained_size(ordinal),
            ));

            fn show_retainers_recursive(
                snap: &HeapSnapshot,
                ordinal: NodeOrdinal,
                depth: usize,
                max_depth: usize,
                offset: usize,
                limit: usize,
                lines: &mut Vec<String>,
            ) {
                let indent = "  ".repeat(depth);
                let retainers = snap.get_retainers(ordinal);
                let total = retainers.len();
                let start = offset.min(total);
                let end = (start + limit).min(total);

                for &(edge_idx, ret_ord) in &retainers[start..end] {
                    let edge_type = snap.edge_type_name(edge_idx);
                    let edge_name = snap.edge_name(edge_idx);
                    let ret_id = snap.node_id(ret_ord);
                    let ret_name = snap.node_display_name(ret_ord);
                    let ret_type = snap.node_type_name(ret_ord);
                    let ret_size = snap.node_self_size(ret_ord);

                    lines.push(format!(
                        "{indent}<--[{edge_type} \"{edge_name}\"]-- @{} {ret_name} (type: {ret_type}, self_size: {ret_size})",
                        ret_id.0
                    ));

                    if depth < max_depth {
                        show_retainers_recursive(snap, ret_ord, depth + 1, max_depth, 0, limit, lines);
                    }
                }

                if end < total {
                    lines.push(format!(
                        "{indent}({}-{} of {total} retainers shown)",
                        start + 1,
                        end
                    ));
                }
            }

            show_retainers_recursive(&snapshot, ordinal, 1, max_depth, offset, limit, &mut lines);

            Ok(CallToolResult::success(vec![Content::text(
                lines.join("\n"),
            )]))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task failed: {e}"), None))?
    }

    #[tool(
        description = "List all native contexts (JavaScript realms) in a snapshot with their label, detachedness, sizes, and unique global variables"
    )]
    async fn get_native_contexts(
        &self,
        Parameters(params): Parameters<GetNativeContextsParams>,
    ) -> Result<CallToolResult, McpError> {
        let snapshot = {
            let snapshots = self.snapshots.lock().await;
            Arc::clone(snapshots.get(&params.snapshot_id).ok_or_else(|| {
                McpError::invalid_params(
                    format!("No snapshot found with id {}", params.snapshot_id),
                    None,
                )
            })?)
        };

        let mut lines = Vec::new();
        let contexts = snapshot.native_contexts();

        for ctx in contexts {
            let ord = ctx.ordinal;
            let id = snapshot.node_id(ord);
            let label = snapshot.native_context_label(ord);
            let det = match snapshot.native_context_detachedness(ord) {
                Detachedness::Attached => "attached",
                Detachedness::Detached => "detached",
                Detachedness::Unknown => "unknown",
            };
            let shallow = snapshot.node_self_size(ord);
            let retained = snapshot.node_retained_size(ord);
            lines.push(format!(
                "@{} {label} (detachedness: {det}, self_size: {shallow}, retained_size: {retained:.0})",
                id.0
            ));
        }

        if lines.is_empty() {
            lines.push("No native contexts found.".to_string());
        } else {
            lines.insert(0, format!("{} native contexts:", contexts.len()));
        }

        Ok(CallToolResult::success(vec![Content::text(
            lines.join("\n"),
        )]))
    }

    #[tool(
        description = "Get memory statistics for a snapshot: total size, V8 heap, native, code, strings, arrays, system, and unreachable objects."
    )]
    async fn get_statistics(
        &self,
        Parameters(params): Parameters<GetStatisticsParams>,
    ) -> Result<CallToolResult, McpError> {
        let snapshot = {
            let snapshots = self.snapshots.lock().await;
            Arc::clone(snapshots.get(&params.snapshot_id).ok_or_else(|| {
                McpError::invalid_params(
                    format!("No snapshot found with id {}", params.snapshot_id),
                    None,
                )
            })?)
        };

        let stats = snapshot.get_statistics();
        let node_count = snapshot.node_count();

        let mut lines = vec![
            format!("{node_count} nodes, {:.0} bytes total", stats.total),
            format!("  V8 heap:      {:.0} bytes", stats.v8heap_total),
            format!("  Native:       {:.0} bytes", stats.native_total),
            format!("  Code:         {:.0} bytes", stats.code),
            format!("  Strings:      {:.0} bytes", stats.strings),
            format!("  JS arrays:    {:.0} bytes", stats.js_arrays),
            format!("  Extra native: {:.0} bytes", stats.extra_native_bytes),
            format!("  Typed arrays: {:.0} bytes", stats.typed_arrays),
            format!("  System:       {:.0} bytes", stats.system),
            format!(
                "  Unreachable:  {:.0} bytes ({} objects)",
                stats.unreachable_size, stats.unreachable_count
            ),
        ];

        let contexts = snapshot.native_contexts();
        if !contexts.is_empty() {
            lines.push(String::new());
            lines.push("Native Context Attribution:".to_string());
            for ctx in contexts {
                let label = snapshot.native_context_label(ctx.ordinal);
                lines.push(format!("  {label}: {:.0} bytes", ctx.size));
            }
            lines.push(format!(
                "  Shared: {:.0} bytes",
                snapshot.shared_attributable_size()
            ));
            lines.push(format!(
                "  Unattributed: {:.0} bytes",
                snapshot.unattributed_size()
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(
            lines.join("\n"),
        )]))
    }

    #[tool(
        description = "Get the containment tree roots: lists all children of the synthetic root (system roots) and all children of (GC roots) (root categories like Strong roots, Global handles, etc.)."
    )]
    async fn get_containment(
        &self,
        Parameters(params): Parameters<GetContainmentParams>,
    ) -> Result<CallToolResult, McpError> {
        let snapshot = {
            let snapshots = self.snapshots.lock().await;
            Arc::clone(snapshots.get(&params.snapshot_id).ok_or_else(|| {
                McpError::invalid_params(
                    format!("No snapshot found with id {}", params.snapshot_id),
                    None,
                )
            })?)
        };

        let mut lines = Vec::new();

        let root = snapshot.synthetic_root_ordinal();
        lines.push("System roots:".to_string());
        for (edge_idx, child_ord) in snapshot.iter_edges(root) {
            if snapshot.root_kind(child_ord) != RootKind::SystemRoot {
                continue;
            }
            let id = snapshot.node_id(child_ord);
            let name = snapshot.node_display_name(child_ord);
            let edge_name = snapshot.edge_name(edge_idx);
            let self_size = snapshot.node_self_size(child_ord);
            let retained = snapshot.node_retained_size(child_ord);
            lines.push(format!(
                "  [{edge_name}] @{} {name} (self_size: {self_size}, retained_size: {retained:.0})",
                id.0
            ));
        }

        let gc_roots = snapshot.gc_roots_ordinal();
        lines.push(String::new());
        lines.push("(GC roots) children:".to_string());
        for (edge_idx, child_ord) in snapshot.iter_edges(gc_roots) {
            let id = snapshot.node_id(child_ord);
            let name = snapshot.node_display_name(child_ord);
            let edge_name = snapshot.edge_name(edge_idx);
            let self_size = snapshot.node_self_size(child_ord);
            let retained = snapshot.node_retained_size(child_ord);
            let child_count = snapshot.node_edge_count(child_ord);
            lines.push(format!(
                "  [{edge_name}] @{} {name} (self_size: {self_size}, retained_size: {retained:.0}, children: {child_count})",
                id.0
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(
            lines.join("\n"),
        )]))
    }

    #[tool(
        description = "Get a summary of objects in the snapshot, grouped by constructor. Shows count, shallow size, and retained size for each group, sorted by retained size descending. Pass a constructor name to expand that group and see individual objects."
    )]
    async fn get_summary(
        &self,
        Parameters(params): Parameters<GetSummaryParams>,
    ) -> Result<CallToolResult, McpError> {
        let snapshot = {
            let snapshots = self.snapshots.lock().await;
            Arc::clone(snapshots.get(&params.snapshot_id).ok_or_else(|| {
                McpError::invalid_params(
                    format!("No snapshot found with id {}", params.snapshot_id),
                    None,
                )
            })?)
        };

        let class_name = params.class_name;
        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(20);
        let filter = params.filter;
        let filter_interval = params.filter_interval;

        tokio::task::spawn_blocking(move || {
            let aggregates = resolve_summary_filter(&snapshot, filter.as_deref(), filter_interval)?;

            if let Some(ref class_name) = class_name {
                let matching: Vec<_> = aggregates
                    .iter()
                    .filter(|a| a.name == *class_name)
                    .collect();
                if matching.is_empty() {
                    return Err(McpError::invalid_params(
                        format!("No constructor group named \"{class_name}\""),
                        None,
                    ));
                }

                let mut lines = Vec::new();
                for entry in &matching {
                    let total = entry.node_ordinals.len();
                    let start = offset.min(total);
                    let end = (start + limit).min(total);

                    lines.push(format!(
                        "{class_name}: {total} objects, {:.0} shallow bytes, {:.0} retained bytes",
                        entry.self_size, entry.max_ret
                    ));
                    lines.push(format!("Showing {}-{} of {total}:", start + 1, end));

                    for &ord in &entry.node_ordinals[start..end] {
                        let id = snapshot.node_id(ord);
                        let name = snapshot.node_display_name(ord);
                        let self_size = snapshot.node_self_size(ord);
                        let retained = snapshot.node_retained_size(ord);
                        lines.push(format!(
                            "  @{} {name} (self_size: {self_size}, retained_size: {retained:.0})",
                            id.0
                        ));
                    }
                }

                Ok(CallToolResult::success(vec![Content::text(
                    lines.join("\n"),
                )]))
            } else {
                let mut entries: Vec<_> = aggregates.iter().collect();
                entries.sort_by(|a, b| {
                    b.max_ret
                        .cmp(&a.max_ret)
                        .then(a.first_seen.cmp(&b.first_seen))
                });

                let mut lines = Vec::new();
                lines.push(format!(
                    "{:<50} {:>8} {:>14} {:>14}",
                    "Constructor", "Count", "Shallow size", "Retained size"
                ));
                for entry in &entries {
                    lines.push(format!(
                        "{:<50} {:>8} {:>14.0} {:>14.0}",
                        entry.name, entry.count, entry.self_size, entry.max_ret
                    ));
                }

                Ok(CallToolResult::success(vec![Content::text(
                    lines.join("\n"),
                )]))
            }
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task failed: {e}"), None))?
    }

    #[tool(
        description = "Walk the dominator tree from an object up to the root, showing the chain of objects that exclusively keep it alive. The object_id should be in the form @12345."
    )]
    async fn get_dominators_of(
        &self,
        Parameters(params): Parameters<GetDominatorsOfParams>,
    ) -> Result<CallToolResult, McpError> {
        let object_id_str = params
            .object_id
            .strip_prefix('@')
            .unwrap_or(&params.object_id);
        let object_id: u64 = object_id_str.parse().map_err(|_| {
            McpError::invalid_params(
                format!(
                    "Invalid object id: {}. Expected format: @12345",
                    params.object_id
                ),
                None,
            )
        })?;

        let snapshot = {
            let snapshots = self.snapshots.lock().await;
            Arc::clone(snapshots.get(&params.snapshot_id).ok_or_else(|| {
                McpError::invalid_params(
                    format!("No snapshot found with id {}", params.snapshot_id),
                    None,
                )
            })?)
        };

        let ordinal = snapshot
            .node_for_snapshot_object_id(NodeId(object_id))
            .ok_or_else(|| {
                McpError::invalid_params(format!("No object found with id @{object_id}"), None)
            })?;

        let mut lines = Vec::new();
        lines.push(format!(
            "Dominator chain for @{object_id}: {} (type: {}, self_size: {}, retained_size: {:.0})",
            snapshot.node_display_name(ordinal),
            snapshot.node_type_name(ordinal),
            snapshot.node_self_size(ordinal),
            snapshot.node_retained_size(ordinal),
        ));

        let mut current = ordinal;
        loop {
            let dom = snapshot.dominator_of(current);
            if dom == current {
                break;
            }
            lines.push(format!(
                "  dominated by {} (type: {}, self_size: {}, retained_size: {:.0})",
                snapshot.format_node_label(dom),
                snapshot.node_type_name(dom),
                snapshot.node_self_size(dom),
                snapshot.node_retained_size(dom),
            ));
            current = dom;
        }

        Ok(CallToolResult::success(vec![Content::text(
            lines.join("\n"),
        )]))
    }

    #[tool(
        description = "Compute the reachable size from a given object: the total shallow size of all objects reachable by following outgoing references. Also returns any native contexts (JavaScript realms) reached."
    )]
    async fn get_reachable_size(
        &self,
        Parameters(params): Parameters<GetReachableSizeParams>,
    ) -> Result<CallToolResult, McpError> {
        let object_id_str = params
            .object_id
            .strip_prefix('@')
            .unwrap_or(&params.object_id);
        let object_id: u64 = object_id_str.parse().map_err(|_| {
            McpError::invalid_params(
                format!(
                    "Invalid object id: {}. Expected format: @12345",
                    params.object_id
                ),
                None,
            )
        })?;

        let snapshot = {
            let snapshots = self.snapshots.lock().await;
            Arc::clone(snapshots.get(&params.snapshot_id).ok_or_else(|| {
                McpError::invalid_params(
                    format!("No snapshot found with id {}", params.snapshot_id),
                    None,
                )
            })?)
        };

        let ordinal = snapshot
            .node_for_snapshot_object_id(NodeId(object_id))
            .ok_or_else(|| {
                McpError::invalid_params(format!("No object found with id @{object_id}"), None)
            })?;

        tokio::task::spawn_blocking(move || {
            let info = snapshot.reachable_size(&[ordinal]);

            let mut lines = Vec::new();
            lines.push(format!(
                "Reachable size from @{object_id} ({}): {:.0} bytes",
                snapshot.node_display_name(ordinal),
                info.size,
            ));

            if info.native_contexts.is_empty() {
                lines.push("No native contexts reached.".to_string());
            } else {
                lines.push(format!(
                    "{} native contexts reached:",
                    info.native_contexts.len()
                ));
                for ctx_ord in &info.native_contexts {
                    let ctx_id = snapshot.node_id(*ctx_ord);
                    let label = snapshot.native_context_label(*ctx_ord);
                    lines.push(format!("  @{} {label}", ctx_id.0));
                }
            }

            Ok(CallToolResult::success(vec![Content::text(
                lines.join("\n"),
            )]))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task failed: {e}"), None))?
    }

    #[tool(
        description = "Find the retaining paths from an object back to GC roots, showing why the object is kept alive. This is the preferred tool for investigating why an object is retained — it automatically traces complete paths to GC roots. The object_id should be in the form @12345."
    )]
    async fn get_retaining_paths(
        &self,
        Parameters(params): Parameters<GetRetainingPathsParams>,
    ) -> Result<CallToolResult, McpError> {
        let object_id_str = params
            .object_id
            .strip_prefix('@')
            .unwrap_or(&params.object_id);
        let object_id: u64 = object_id_str.parse().map_err(|_| {
            McpError::invalid_params(
                format!(
                    "Invalid object id: {}. Expected format: @12345",
                    params.object_id
                ),
                None,
            )
        })?;

        let snapshot = {
            let snapshots = self.snapshots.lock().await;
            Arc::clone(snapshots.get(&params.snapshot_id).ok_or_else(|| {
                McpError::invalid_params(
                    format!("No snapshot found with id {}", params.snapshot_id),
                    None,
                )
            })?)
        };

        let ordinal = snapshot
            .node_for_snapshot_object_id(NodeId(object_id))
            .ok_or_else(|| {
                McpError::invalid_params(format!("No object found with id @{object_id}"), None)
            })?;

        let max_depth = params.max_depth.unwrap_or(50);
        let max_nodes = params.max_nodes.unwrap_or(200);

        tokio::task::spawn_blocking(move || {
            let plan = plan_gc_root_retainer_paths(
                &snapshot,
                ordinal,
                RetainerAutoExpandLimits {
                    max_depth,
                    max_nodes,
                },
            );

            let mut lines = Vec::new();
            lines.push(format!(
                "Retaining paths for @{object_id}: {} (type: {}, self_size: {}, retained_size: {:.0})",
                snapshot.node_display_name(ordinal),
                snapshot.node_type_name(ordinal),
                snapshot.node_self_size(ordinal),
                snapshot.node_retained_size(ordinal),
            ));

            fn format_edge(snap: &HeapSnapshot, edge_idx: usize, ret_ordinal: NodeOrdinal) -> String {
                let edge_type = snap.edge_type_name(edge_idx);
                let edge_name = snap.edge_name(edge_idx);
                let node_name = snap.node_display_name(ret_ordinal);
                let node_id = snap.node_id(ret_ordinal);
                let node_type = snap.node_type_name(ret_ordinal);
                let distance = snap.node_distance(ret_ordinal);

                let label = if edge_type == "element" || edge_type == "hidden" {
                    format!("[{edge_name}]")
                } else {
                    edge_name.to_string()
                };

                format!(
                    "--[{label}]--> @{} {node_name} (type: {node_type}, distance: {distance})",
                    node_id.0
                )
            }

            fn walk(
                snap: &HeapSnapshot,
                edges: &[RetainerPathEdge],
                depth: usize,
                lines: &mut Vec<String>,
            ) {
                for pe in edges {
                    let indent = "  ".repeat(depth);
                    lines.push(format!(
                        "{indent}{}",
                        format_edge(snap, pe.edge_idx, pe.retainer)
                    ));
                    walk(snap, &pe.children, depth + 1, lines);
                }
            }

            if plan.truncated || (!plan.reached_gc_roots && !snapshot.is_root(ordinal)) {
                lines.push(String::new());
                walk(&snapshot, &plan.tree, 0, &mut lines);
                return Err(McpError::internal_error(lines.join("\n"), None));
            }

            lines.push(String::new());
            walk(&snapshot, &plan.tree, 0, &mut lines);

            Ok(CallToolResult::success(vec![Content::text(
                lines.join("\n"),
            )]))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task failed: {e}"), None))?
    }

    #[tool(
        description = "Compare two snapshots, showing objects that were added or removed between the baseline and the main snapshot. Returns per-constructor diffs with counts and sizes. Pass a class_name to see individual objects for that constructor."
    )]
    async fn compare_snapshots(
        &self,
        Parameters(params): Parameters<CompareSnapshotsParams>,
    ) -> Result<CallToolResult, McpError> {
        let (snapshot, baseline) = {
            let snapshots = self.snapshots.lock().await;
            let snapshot = Arc::clone(snapshots.get(&params.snapshot_id).ok_or_else(|| {
                McpError::invalid_params(
                    format!("No snapshot found with id {}", params.snapshot_id),
                    None,
                )
            })?);
            let baseline = Arc::clone(snapshots.get(&params.baseline_id).ok_or_else(|| {
                McpError::invalid_params(
                    format!("No snapshot found with id {}", params.baseline_id),
                    None,
                )
            })?);
            (snapshot, baseline)
        };

        let class_name = params.class_name;
        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(20);

        tokio::task::spawn_blocking(move || {
            let diffs = diff::compute_diff(&snapshot, &baseline);

            if let Some(ref class_name) = class_name {
                let matching: Vec<_> = diffs.iter()
                    .filter(|d| d.name == *class_name)
                    .collect();
                if matching.is_empty() {
                    return Err(McpError::invalid_params(
                        format!("No diff entry for constructor \"{class_name}\""),
                        None,
                    ));
                }

                let mut lines = Vec::new();
                for entry in &matching {
                    lines.push(format!(
                        "{class_name}: # new: {}, # deleted: {}, # delta: {}, alloc size: {}, freed size: {}, size delta: {}",
                        entry.new_count,
                        entry.deleted_count,
                        format_signed_count(entry.delta_count()),
                        format_signed_size(entry.alloc_size as i64),
                        format_signed_size(entry.freed_size as i64),
                        format_signed_size(entry.size_delta()),
                    ));

                    let all_objects: Vec<(bool, &NodeId, &u32)> = entry
                        .new_objects
                        .iter()
                        .map(|(id, sz)| (true, id, sz))
                        .chain(entry.deleted_objects.iter().map(|(id, sz)| (false, id, sz)))
                        .collect();
                    let total = all_objects.len();
                    let start = offset.min(total);
                    let end = (start + limit).min(total);

                    lines.push(format!("Showing {}-{} of {total} objects:", start + 1, end));
                    for &(is_new, node_id, self_size) in &all_objects[start..end] {
                        let status = if is_new { "+" } else { "\u{2212}" };
                        lines.push(format!(
                            "  {status} @{} (self_size: {self_size})",
                            node_id.0
                        ));
                    }
                }

                Ok(CallToolResult::success(vec![Content::text(
                    lines.join("\n"),
                )]))
            } else {
                let total = diffs.len();
                let start = offset.min(total);
                let end = (start + limit).min(total);

                let mut lines = Vec::new();
                lines.push(format!(
                    "{total} constructors with changes. Showing {}-{}:",
                    start + 1,
                    end
                ));
                lines.push(format!(
                    "{:<50} {:>8} {:>10} {:>8} {:>14} {:>14} {:>14}",
                    "Constructor", "# New", "# Deleted", "# Delta", "Alloc. Size", "Freed Size", "Size Delta"
                ));
                for diff in &diffs[start..end] {
                    lines.push(format!(
                        "{:<50} {:>8} {:>10} {:>8} {:>14} {:>14} {:>14}",
                        diff.name,
                        diff.new_count,
                        diff.deleted_count,
                        format_signed_count(diff.delta_count()),
                        format_signed_size(diff.alloc_size as i64),
                        format_signed_size(diff.freed_size as i64),
                        format_signed_size(diff.size_delta()),
                    ));
                }

                Ok(CallToolResult::success(vec![Content::text(
                    lines.join("\n"),
                )]))
            }
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task failed: {e}"), None))?
    }

    #[tool(
        description = "Find duplicate strings in the heap. Shows strings that appear more than once, sorted by wasted bytes (total size minus one instance). Returns the string value, count, instance size, total size, and wasted bytes for each duplicate."
    )]
    async fn get_duplicate_strings(
        &self,
        Parameters(params): Parameters<GetDuplicateStringsParams>,
    ) -> Result<CallToolResult, McpError> {
        let snapshot = {
            let snapshots = self.snapshots.lock().await;
            Arc::clone(snapshots.get(&params.snapshot_id).ok_or_else(|| {
                McpError::invalid_params(
                    format!("No snapshot found with id {}", params.snapshot_id),
                    None,
                )
            })?)
        };

        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(100);
        let show_object_ids = params.show_object_ids.unwrap_or(false);

        tokio::task::spawn_blocking(move || {
            let result = snapshot.duplicate_strings();
            let duplicates = &result.duplicates;
            let total = duplicates.len();
            let total_wasted: u64 = duplicates.iter().map(|d| d.wasted_size()).sum();
            let start = offset.min(total);
            let end = (start + limit).min(total);

            let mut lines = Vec::new();
            lines.push(format!(
                "{total} duplicate string groups, {total_wasted} bytes wasted total"
            ));
            if result.skipped_count > 0 {
                lines.push(format!(
                    "({} strings, {} bytes skipped — no length metadata)",
                    result.skipped_count, result.skipped_size
                ));
            }
            lines.push(format!("Showing entries {start}..{end}:"));
            lines.push(String::new());

            for entry in &duplicates[start..end] {
                let mut display = truncate_str(&entry.value, 80);
                if entry.truncated {
                    display = format!("{display}... (len {})", entry.length);
                }
                lines.push(format!(
                    "\"{}\" x{} (instance_size: {}, total: {}, wasted: {})",
                    display,
                    entry.count,
                    entry.instance_size,
                    entry.total_size,
                    entry.wasted_size(),
                ));
                if show_object_ids {
                    let ids = entry
                        .node_ids
                        .iter()
                        .map(|id| format!("@{}", id.0))
                        .collect::<Vec<_>>()
                        .join(", ");
                    lines.push(format!("  ids: {ids}"));
                }
            }

            if end < total {
                lines.push(String::new());
                lines.push(format!(
                    "Use offset={end} to see more entries ({} remaining).",
                    total - end
                ));
            }

            Ok(CallToolResult::success(vec![Content::text(
                lines.join("\n"),
            )]))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task failed: {e}"), None))?
    }

    #[tool(
        description = "Detect closure context leaks: finds contexts where some variables are not accessed by any live closure, indicating unnecessarily retained data. This is a common JavaScript memory leak pattern where V8 shares a single context per scope, so a live closure retains all variables from its scope even if it only uses some of them. Results are sorted by retained size."
    )]
    async fn get_closure_leaks(
        &self,
        Parameters(params): Parameters<GetClosureLeaksParams>,
    ) -> Result<CallToolResult, McpError> {
        let snapshot = {
            let snapshots = self.snapshots.lock().await;
            Arc::clone(snapshots.get(&params.snapshot_id).ok_or_else(|| {
                McpError::invalid_params(
                    format!("No snapshot found with id {}", params.snapshot_id),
                    None,
                )
            })?)
        };

        let show_incomplete = params.show_incomplete.unwrap_or(false);

        tokio::task::spawn_blocking(move || {
            let contexts = closure_leaks::collect_contexts(&snapshot, false, false, false);
            let mut leaks = closure_leaks::find_closure_leaks(&snapshot, &contexts);

            if !show_incomplete {
                leaks.retain(|l| matches!(&l.result, closure_leaks::UnusedVarsResult::Complete(_)));
            }

            leaks.sort_by(|a, b| {
                snapshot
                    .node_retained_size(b.context_ord)
                    .cmp(&snapshot.node_retained_size(a.context_ord))
            });

            let mut lines = Vec::new();

            if leaks.is_empty() {
                lines.push("No closure leaks detected.".to_string());
            } else {
                for leak in &leaks {
                    let id = snapshot.node_id(leak.context_ord);
                    let retained = snapshot.node_retained_size(leak.context_ord);
                    let all_vars = snapshot.context_variable_names(leak.context_ord);

                    lines.push(format!(
                        "@{id} (retained: {})  vars: [{}]",
                        crate::print::format_size(retained),
                        all_vars.join(", "),
                    ));

                    match &leak.result {
                        closure_leaks::UnusedVarsResult::Incomplete(reason) => {
                            lines.push(format!("  (incomplete: {reason})"));
                        }
                        closure_leaks::UnusedVarsResult::Complete(unused) => {
                            for name in unused {
                                let target_info = snapshot
                                    .iter_edges(leak.context_ord)
                                    .find(|&(ei, _)| {
                                        snapshot.edge_type_name(ei) == "context"
                                            && snapshot.edge_name(ei) == *name
                                    })
                                    .map(|(_, child_ord)| {
                                        let child_id = snapshot.node_id(child_ord);
                                        let child_name = snapshot.node_display_name(child_ord);
                                        let child_retained = snapshot.node_retained_size(child_ord);
                                        format!(
                                            ": @{child_id} {child_name} (retained: {})",
                                            crate::print::format_size(child_retained),
                                        )
                                    })
                                    .unwrap_or_default();
                                lines.push(format!("  unused: {name}{target_info}"));
                            }
                        }
                    }
                }

                lines.push(format!("\n{} contexts with unused variables", leaks.len()));
            }

            Ok(CallToolResult::success(vec![Content::text(
                lines.join("\n"),
            )]))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task failed: {e}"), None))?
    }

    #[tool(
        description = "Show the allocation timeline of the heap snapshot. Each interval shows a timestamp, the size of live objects allocated, and the number of objects. Returns a message if no timeline data is available."
    )]
    async fn get_timeline(
        &self,
        Parameters(params): Parameters<GetTimelineParams>,
    ) -> Result<CallToolResult, McpError> {
        let snapshot = {
            let snapshots = self.snapshots.lock().await;
            Arc::clone(snapshots.get(&params.snapshot_id).ok_or_else(|| {
                McpError::invalid_params(
                    format!("No snapshot found with id {}", params.snapshot_id),
                    None,
                )
            })?)
        };

        tokio::task::spawn_blocking(move || {
            let intervals = snapshot.get_timeline();
            if intervals.is_empty() {
                return Ok(CallToolResult::success(vec![Content::text(
                    "No allocation timeline data in this snapshot.",
                )]));
            }

            let total_count: u64 = intervals.iter().map(|i| i.count as u64).sum();
            let total_size: u64 = intervals.iter().map(|i| i.size).sum();

            let mut lines = Vec::new();
            lines.push(format!(
                "Allocation Timeline ({} intervals, {} live objects, {} total):",
                intervals.len(),
                total_count,
                format_size(total_size),
            ));
            lines.push(String::new());

            for interval in intervals {
                let ts_sec = interval.timestamp_us as f64 / 1_000_000.0;
                lines.push(format!(
                    "  {:>6.1}s  {:>8}  {:>5} obj  ids {}..{}",
                    ts_sec,
                    format_size(interval.size),
                    interval.count,
                    interval.id_from,
                    interval.id_to,
                ));
            }

            Ok(CallToolResult::success(vec![Content::text(
                lines.join("\n"),
            )]))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("Task failed: {e}"), None))?
    }
}

fn resolve_summary_filter(
    snapshot: &HeapSnapshot,
    filter: Option<&str>,
    filter_interval: Option<usize>,
) -> Result<AggregateMap, McpError> {
    if let Some(idx) = filter_interval {
        let intervals = snapshot.get_timeline();
        if idx >= intervals.len() {
            return Err(McpError::invalid_params(
                format!(
                    "Invalid interval index {idx}, snapshot has {} intervals",
                    intervals.len()
                ),
                None,
            ));
        }
        let interval = &intervals[idx];
        return Ok(snapshot.aggregates_for_id_range(interval.id_from, interval.id_to));
    }
    match filter {
        None | Some("") => Ok(snapshot.aggregates_with_filter()),
        Some("attached") => Ok(snapshot.aggregates_attached()),
        Some("detached") => Ok(snapshot.aggregates_detached()),
        Some("unreachable") => Ok(snapshot.unreachable_aggregates()),
        Some("unreachable-roots") => Ok(snapshot.unreachable_root_aggregates()),
        Some("detached-dom") => Ok(snapshot.retained_by_detached_dom()),
        Some("console") => Ok(snapshot.retained_by_console()),
        Some("event-handlers") => Ok(snapshot.retained_by_event_handlers()),
        Some(other) => Err(McpError::invalid_params(
            format!(
                "Unknown filter '{other}'. Valid filters: attached, detached, unreachable, unreachable-roots, detached-dom, console, event-handlers"
            ),
            None,
        )),
    }
}

// ---------------------------------------------------------------------------
// ServerHandler impl
// ---------------------------------------------------------------------------

#[tool_handler]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "heap-snapshot-mcp",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Heap snapshot analysis server. Load a .heapsnapshot file, \
                 then inspect objects and their outgoing references."
                    .to_string(),
            )
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParams,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        Ok(self.get_info())
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    use rmcp::{ServiceExt, transport::stdio};

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        // Logs go to stderr since stdout is the MCP transport
        eprintln!("heap-snapshot-mcp starting...");

        let service = McpServer::new().serve(stdio()).await?;
        service.waiting().await?;

        Ok(())
    })
}
