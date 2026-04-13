import init, { WasmHeapSnapshot } from '../../wasm-pkg/heap_snapshot_wasm.js';

let initialized = false;
const snapshots = new Map<number, WasmHeapSnapshot>();
let nextSnapshotId = 1;

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

function getSnapshot(snapshotId: number): WasmHeapSnapshot {
  const snap = snapshots.get(snapshotId);
  if (!snap) throw new Error(`No snapshot with id ${snapshotId}`);
  return snap;
}

// ---------------------------------------------------------------------------
// Priority queue: regular messages are processed immediately.
// Background messages (e.g. auto-computed reachable sizes) are deferred
// and only processed when no regular messages are pending.
// ---------------------------------------------------------------------------

const regularQueue: WorkerMsg[] = [];
const backgroundQueue: WorkerMsg[] = [];
let processing = false;

async function drainQueues() {
  if (processing) return;
  processing = true;
  while (regularQueue.length > 0 || backgroundQueue.length > 0) {
    // Always prefer regular requests over background ones.
    const msg =
      regularQueue.length > 0
        ? regularQueue.shift()!
        : backgroundQueue.shift()!;
    await processMessage(msg);
  }
  processing = false;
}

self.onmessage = (e: MessageEvent<WorkerMsg>) => {
  const msg = e.data;
  if (msg.background) {
    backgroundQueue.push(msg);
  } else {
    regularQueue.push(msg);
  }
  drainQueues();
};

async function processMessage(msg: WorkerMsg) {
  const { id } = msg;

  try {
    if (msg.type === 'computeDiff') {
      const main = getSnapshot(msg.snapshotId as number);
      const baseline = getSnapshot(msg.baselineId as number);
      respond(id, JSON.parse(main.compute_diff(baseline)));
      return;
    }

    if (msg.type === 'close') {
      const snap = snapshots.get(msg.snapshotId as number);
      if (snap) {
        snap.free();
        snapshots.delete(msg.snapshotId as number);
      }
      respond(id, null);
      return;
    }

    if (msg.type === 'load') {
      if (!initialized) {
        await init();
        initialized = true;
      }
      const bytes = new Uint8Array(msg.data as ArrayBuffer);
      const snapshot = new WasmHeapSnapshot(bytes);
      const snapshotId = nextSnapshotId++;
      snapshots.set(snapshotId, snapshot);
      respond(id, {
        snapshotId,
        nodeCount: snapshot.node_count(),
        hasAllocationData: snapshot.has_allocation_data(),
      });
      return;
    }

    const snapshot = getSnapshot(msg.snapshotId as number);

    switch (msg.type) {
      case 'getStatistics':
        respond(id, JSON.parse(snapshot.get_statistics()));
        break;
      case 'getSummaryWithFilter':
        respond(
          id,
          JSON.parse(
            snapshot.get_summary_with_filter((msg.mode as number) || 0),
          ),
        );
        break;
      case 'getSummaryWithContextFilter':
        respond(
          id,
          JSON.parse(
            snapshot.get_summary_with_context_filter(
              msg.contextMode as number,
              msg.contextIndex as number,
            ),
          ),
        );
        break;
      case 'getSummaryObjects':
        respond(
          id,
          JSON.parse(
            snapshot.get_summary_objects(
              msg.constructorIndex as number,
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
              (msg.filter as string) || '',
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
              (msg.filter as string) || '',
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
      case 'getSummaryObjectIndex':
        respond(
          id,
          JSON.parse(
            snapshot.get_summary_object_index(
              msg.constructorIndex as number,
              msg.nodeId as number,
            ),
          ),
        );
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
      case 'getTimeline':
        respond(id, JSON.parse(snapshot.get_timeline()));
        break;
      case 'getSummaryForInterval':
        respond(
          id,
          JSON.parse(
            snapshot.get_summary_for_interval(msg.intervalIndex as number),
          ),
        );
        break;
      case 'getTimelineObjects':
        respond(
          id,
          JSON.parse(
            snapshot.get_timeline_objects(
              msg.constructorIndex as number,
              msg.offset as number,
              msg.limit as number,
            ),
          ),
        );
        break;
      case 'getAllocationStack': {
        const result = snapshot.get_allocation_stack(msg.nodeId as number);
        respond(id, result === 'null' ? null : JSON.parse(result));
        break;
      }
    }
  } catch (err) {
    respondError(id, String(err));
  }
}
