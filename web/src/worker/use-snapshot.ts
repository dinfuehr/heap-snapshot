import { useRef, useState, useCallback } from 'react';
import type { WorkerResponse } from '../types.ts';

type PendingRequest = {
  resolve: (data: unknown) => void;
  reject: (error: Error) => void;
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type SnapshotCall = <T = unknown>(
  request: Record<string, any>,
) => Promise<T>;

export function useSnapshot() {
  const workerRef = useRef<Worker | null>(null);
  const nextIdRef = useRef(1);
  const pendingRef = useRef<Map<number, PendingRequest>>(new Map());
  const [loading, setLoading] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const getWorker = useCallback(() => {
    if (!workerRef.current) {
      const worker = new Worker(
        new URL('./snapshot-worker.ts', import.meta.url),
        { type: 'module' },
      );
      worker.onmessage = (e: MessageEvent<WorkerResponse>) => {
        const msg = e.data;
        if ('id' in msg) {
          const pending = pendingRef.current.get(msg.id);
          if (pending) {
            pendingRef.current.delete(msg.id);
            if (msg.type === 'success') {
              pending.resolve(msg.data);
            } else {
              pending.reject(new Error(msg.error));
            }
          }
        }
      };
      workerRef.current = worker;
    }
    return workerRef.current;
  }, []);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const call: SnapshotCall = useCallback(
    <T = unknown>(request: Record<string, any>): Promise<T> => {
      const worker = getWorker();
      const id = nextIdRef.current++;
      return new Promise<T>((resolve, reject) => {
        pendingRef.current.set(id, {
          resolve: resolve as (data: unknown) => void,
          reject,
        });
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const msg: any = { ...request, id };
        if (msg.type === 'load' && msg.data instanceof ArrayBuffer) {
          worker.postMessage(msg, [msg.data]);
        } else {
          worker.postMessage(msg);
        }
      });
    },
    [getWorker],
  );

  const loadFile = useCallback(
    async (file: File) => {
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
    },
    [call],
  );

  return { loading, loaded, error, loadFile, call };
}
