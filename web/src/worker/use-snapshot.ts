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

export function createSnapshot() {
  let worker: Worker | null = null;
  let nextId = 1;
  const pending = new Map<number, PendingRequest>();
  const [loading, setLoading] = createSignal(false);
  const [loaded, setLoaded] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  function getWorker() {
    if (!worker) {
      worker = new Worker(new URL('./snapshot-worker.ts', import.meta.url), {
        type: 'module',
      });
      worker.onmessage = (e: MessageEvent<WorkerResponse>) => {
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
    return worker;
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const call: SnapshotCall = <T = unknown>(
    request: Record<string, any>,
  ): Promise<T> => {
    const w = getWorker();
    const id = nextId++;
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
  };

  async function loadFile(file: File) {
    setLoading(true);
    setError(null);
    try {
      const buffer = await file.arrayBuffer();
      await call({ type: 'load', data: buffer });
      setLoaded(true);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  return { loading, loaded, error, loadFile, call };
}
