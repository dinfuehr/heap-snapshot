import { createSignal } from 'solid-js';
import type { WorkerResponse } from '../types.ts';

type PendingRequest = {
  resolve: (data: unknown) => void;
  reject: (error: Error) => void;
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type SnapshotCall = <T = unknown>(
  request: Record<string, any>,
) => Promise<T>;

// ---------------------------------------------------------------------------
// Shared worker singleton
// ---------------------------------------------------------------------------

let sharedWorker: Worker | null = null;
let nextMsgId = 1;
const pending = new Map<number, PendingRequest>();

function getWorker(): Worker {
  if (!sharedWorker) {
    sharedWorker = new Worker(
      new URL('./snapshot-worker.ts', import.meta.url),
      { type: 'module' },
    );
    sharedWorker.onmessage = (e: MessageEvent<WorkerResponse>) => {
      const msg = e.data;
      if ('id' in msg) {
        const p = pending.get(msg.id);
        if (p) {
          pending.delete(msg.id);
          if (msg.type === 'success') {
            p.resolve(msg.data);
          } else {
            p.reject(new Error(msg.error));
          }
        }
      }
    };
  }
  return sharedWorker;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function workerCall<T = unknown>(
  request: Record<string, any>,
): Promise<T> {
  const w = getWorker();
  const id = nextMsgId++;
  return new Promise<T>((resolve, reject) => {
    pending.set(id, {
      resolve: resolve as (data: unknown) => void,
      reject,
    });
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const msg: any = { ...request, id };
    if (msg.type === 'load' && msg.data instanceof ArrayBuffer) {
      w.postMessage(msg, [msg.data]);
    } else {
      w.postMessage(msg);
    }
  });
}

// ---------------------------------------------------------------------------
// Per-snapshot instance
// ---------------------------------------------------------------------------

export function createSnapshot() {
  const [loading, setLoading] = createSignal(false);
  const [loaded, setLoaded] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [filename, setFilename] = createSignal<string | null>(null);
  const [hasAllocationData, setHasAllocationData] = createSignal(false);
  const [snapshotId, setSnapshotId] = createSignal<number | null>(null);

  // Bound call that injects this snapshot's ID into every request.
  const call: SnapshotCall = <T = unknown>(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    request: Record<string, any>,
  ): Promise<T> => {
    const id = snapshotId();
    if (id === null) {
      return Promise.reject(new Error('No snapshot loaded'));
    }
    return workerCall<T>({ ...request, snapshotId: id });
  };

  async function loadFile(file: File) {
    setLoading(true);
    setError(null);
    setFilename(file.name);
    try {
      const buffer = await file.arrayBuffer();
      const result = await workerCall<{
        snapshotId: number;
        nodeCount: number;
        hasAllocationData: boolean;
      }>({ type: 'load', data: buffer });
      setSnapshotId(result.snapshotId);
      setHasAllocationData(result.hasAllocationData);
      setLoaded(true);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  async function close() {
    const id = snapshotId();
    if (id !== null) {
      await workerCall({ type: 'close', snapshotId: id });
      setSnapshotId(null);
      setLoaded(false);
      setFilename(null);
      setHasAllocationData(false);
    }
  }

  return {
    loading,
    loaded,
    error,
    filename,
    hasAllocationData,
    snapshotId,
    loadFile,
    close,
    call,
  };
}
