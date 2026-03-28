use std::collections::HashMap;
use std::fs::File;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::schemars;
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler};

use heap_snapshot::parser;
use heap_snapshot::snapshot::{HeapSnapshot, SnapshotOptions};
use heap_snapshot::types::{NodeId, NodeOrdinal};

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
struct GetOutgoingReferencesParams {
    /// Snapshot ID returned by load_snapshot
    snapshot_id: u32,
    /// Object ID in the form @12345
    object_id: String,
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

// ---------------------------------------------------------------------------
// Server state
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct McpServer {
    snapshots: Arc<Mutex<HashMap<u32, Arc<HeapSnapshot>>>>,
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
            snapshots: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU32::new(1)),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Load a .heapsnapshot file and return a snapshot_id for use with other tools")]
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
        description = "Get the outgoing references (edges) for an object. The object_id should be in the form @12345."
    )]
    async fn get_outgoing_references(
        &self,
        Parameters(params): Parameters<GetOutgoingReferencesParams>,
    ) -> Result<CallToolResult, McpError> {
        let object_id_str = params.object_id.strip_prefix('@').unwrap_or(&params.object_id);
        let object_id: u64 = object_id_str.parse().map_err(|_| {
            McpError::invalid_params(
                format!("Invalid object id: {}. Expected format: @12345", params.object_id),
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
                McpError::invalid_params(
                    format!("No object found with id @{object_id}"),
                    None,
                )
            })?;

        let mut lines = Vec::new();
        lines.push(format!(
            "Object @{object_id}: {} (type: {}, self_size: {})",
            snapshot.node_display_name(ordinal),
            snapshot.node_type_name(ordinal),
            snapshot.node_self_size(ordinal),
        ));
        lines.push(String::new());

        let mut edge_count = 0;
        for (edge_idx, child_ord) in snapshot.iter_edges(ordinal) {
            let edge_type = snapshot.edge_type_name(edge_idx);
            let edge_name = snapshot.edge_name(edge_idx);
            let child_id = snapshot.node_id(child_ord);
            let child_name = snapshot.node_display_name(child_ord);
            let child_type = snapshot.node_type_name(child_ord);
            let child_size = snapshot.node_self_size(child_ord);

            lines.push(format!(
                "  --[{edge_type} \"{edge_name}\"]--> @{} {} (type: {child_type}, self_size: {child_size})",
                child_id.0, child_name
            ));
            edge_count += 1;
        }

        if edge_count == 0 {
            lines.push("  (no outgoing references)".to_string());
        } else {
            lines.insert(1, format!("{edge_count} outgoing references:"));
        }

        Ok(CallToolResult::success(vec![Content::text(
            lines.join("\n"),
        )]))
    }

    #[tool(description = "List all native contexts (JavaScript realms) in a snapshot with their label, detachedness, sizes, and unique global variables")]
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

        for &ord in contexts {
            let ord = NodeOrdinal(ord);
            let id = snapshot.node_id(ord);
            let label = snapshot.native_context_label(ord);
            let det = match snapshot.native_context_detachedness(ord) {
                1 => "attached",
                2 => "detached",
                _ => "unknown",
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
        description = "Compute the reachable size from a given object: the total shallow size of all objects reachable by following outgoing references. Also returns any native contexts (JavaScript realms) reached."
    )]
    async fn get_reachable_size(
        &self,
        Parameters(params): Parameters<GetReachableSizeParams>,
    ) -> Result<CallToolResult, McpError> {
        let object_id_str = params.object_id.strip_prefix('@').unwrap_or(&params.object_id);
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
                McpError::invalid_params(
                    format!("No object found with id @{object_id}"),
                    None,
                )
            })?;

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
    }
}

// ---------------------------------------------------------------------------
// ServerHandler impl
// ---------------------------------------------------------------------------

#[tool_handler]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("heap-snapshot-mcp", env!("CARGO_PKG_VERSION")))
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use rmcp::{transport::stdio, ServiceExt};

    // Logs go to stderr since stdout is the MCP transport
    eprintln!("heap-snapshot-mcp starting...");

    let service = McpServer::new().serve(stdio()).await?;
    service.waiting().await?;

    Ok(())
}
