import init, { WasmHeapSnapshot } from '../../wasm-pkg/heap_snapshot_wasm.js';

let snapshot: WasmHeapSnapshot | null = null;
let initialized = false;

interface WorkerMsg {
  id: number;
  type: string;
  [key: string]: unknown;
}

interface WorkerResponse {
  id: number;
  type: 'success' | 'error';
  data?: unknown;
  error?: string;
}

function respond(id: number, data: unknown) {
  self.postMessage({ id, type: 'success', data } satisfies WorkerResponse);
}

function respondError(id: number, error: string) {
  self.postMessage({ id, type: 'error', error } satisfies WorkerResponse);
}

self.onmessage = async (e: MessageEvent<WorkerMsg>) => {
  const msg = e.data;
  const { id } = msg;

  try {
    if (msg.type === 'load') {
      if (!initialized) {
        await init();
        initialized = true;
      }
      const bytes = new Uint8Array(msg.data as ArrayBuffer);
      snapshot = new WasmHeapSnapshot(bytes);
      respond(id, { nodeCount: snapshot.node_count() });
      return;
    }

    if (!snapshot) {
      respondError(id, 'No snapshot loaded');
      return;
    }

    switch (msg.type) {
      case 'getStatistics':
        respond(id, JSON.parse(snapshot.get_statistics()));
        break;
      case 'getSummary':
        respond(id, JSON.parse(snapshot.get_summary()));
        break;
      case 'getSummaryObjects':
        respond(
          id,
          JSON.parse(
            snapshot.get_summary_objects(
              String(msg['constructor']),
              msg.offset as number,
              msg.limit as number,
            ),
          ),
        );
        break;
      case 'getContainment':
        respond(id, JSON.parse(snapshot.get_containment()));
        break;
      case 'getChildren':
        respond(
          id,
          JSON.parse(
            snapshot.get_children(
              msg.nodeId as number,
              msg.offset as number,
              msg.limit as number,
            ),
          ),
        );
        break;
      case 'getRetainers':
        respond(
          id,
          JSON.parse(
            snapshot.get_retainers(
              msg.nodeId as number,
              msg.offset as number,
              msg.limit as number,
            ),
          ),
        );
        break;
      case 'getRetainingPaths':
        respond(
          id,
          JSON.parse(
            snapshot.get_retaining_paths(
              msg.nodeId as number,
              msg.maxDepth as number,
              msg.maxNodes as number,
            ),
          ),
        );
        break;
      case 'getNativeContexts':
        respond(id, JSON.parse(snapshot.get_native_contexts()));
        break;
      case 'getDominatorsOf':
        respond(
          id,
          JSON.parse(snapshot.get_dominators_of(msg.nodeId as number)),
        );
        break;
      case 'getDominatedChildren':
        respond(
          id,
          JSON.parse(
            snapshot.get_dominated_children(
              msg.nodeId as number,
              msg.offset as number,
              msg.limit as number,
            ),
          ),
        );
        break;
      case 'getDominatorTreeRoot':
        respond(id, JSON.parse(snapshot.get_dominator_tree_root()));
        break;
      case 'getNodeInfo':
        respond(id, JSON.parse(snapshot.get_node_info(msg.nodeId as number)));
        break;
      case 'getConstructorForNode':
        respond(id, snapshot.get_constructor_for_node(msg.nodeId as number));
        break;
      case 'getReachableSize':
        respond(
          id,
          JSON.parse(snapshot.get_reachable_size(msg.nodeId as number)),
        );
        break;
      case 'getChildrenIds':
        respond(
          id,
          JSON.parse(snapshot.get_children_ids(msg.nodeId as number)),
        );
        break;
    }
  } catch (err) {
    respondError(id, String(err));
  }
};
