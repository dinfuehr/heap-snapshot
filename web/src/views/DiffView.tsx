import { createSignal, Show, For, type JSX, type Accessor } from 'solid-js';
import type { ClassDiff } from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import { workerCall } from '../worker/use-snapshot.ts';
import { formatBytes } from '../components/format.ts';

type SnapshotInstance = {
  loaded: () => boolean;
  filename: () => string | null;
  snapshotId: () => number | null;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  [key: string]: any;
};

function formatSigned(bytes: number): string {
  if (bytes === 0) return '0 B';
  const sign = bytes > 0 ? '+' : '';
  return sign + formatBytes(Math.abs(bytes));
}

function formatSignedCount(n: number): string {
  if (n === 0) return '0';
  return n > 0 ? `+${n.toLocaleString()}` : n.toLocaleString();
}

const numTd = {
  padding: '3px 8px',
  'text-align': 'right' as const,
  'font-variant-numeric': 'tabular-nums',
  'white-space': 'nowrap' as const,
};

export function DiffView(props: {
  call: SnapshotCall;
  snapshotId: () => number | null;
  snapshots: Accessor<SnapshotInstance[]>;
  currentIndex: Accessor<number>;
}): JSX.Element {
  const [baselineId, setBaselineId] = createSignal<number | null>(null);
  const [diffs, setDiffs] = createSignal<ClassDiff[] | null>(null);
  const [loading, setLoading] = createSignal(false);

  const otherSnapshots = () =>
    props
      .snapshots()
      .filter(
        (s) =>
          s.snapshotId() !== props.snapshotId() &&
          s.snapshotId() !== null &&
          s.loaded(),
      );

  const computeDiff = async (otherSnapshotId: number) => {
    setBaselineId(otherSnapshotId);
    setDiffs(null);
    setLoading(true);

    const mainId = props.snapshotId();
    if (mainId === null) {
      setLoading(false);
      return;
    }

    const result = await workerCall<ClassDiff[]>({
      type: 'computeDiff',
      snapshotId: mainId,
      baselineId: otherSnapshotId,
    });
    setDiffs(result);
    setLoading(false);
  };

  return (
    <div>
      <div
        style={{
          'margin-bottom': '12px',
          display: 'flex',
          'align-items': 'center',
          gap: '8px',
          'font-size': '14px',
        }}
      >
        <span>Compare against:</span>
        <select
          data-testid="diff-baseline-select"
          value={baselineId() ?? ''}
          onChange={(e) => {
            const val = e.currentTarget.value;
            if (val !== '') computeDiff(parseInt(val, 10));
          }}
          style={{ padding: '4px 8px', 'font-size': '13px' }}
        >
          <option value="">Select a snapshot...</option>
          <For each={otherSnapshots()}>
            {(s) => (
              <option value={s.snapshotId()!}>
                {s.filename() ?? `Snapshot ${s.snapshotId()}`}
              </option>
            )}
          </For>
        </select>
        <Show when={loading()}>
          <span style={{ color: '#888', 'font-size': '12px' }}>
            Computing diff...
          </span>
        </Show>
      </div>

      <Show when={diffs() !== null && diffs()!.length === 0 && !loading()}>
        <p style={{ color: '#888' }}>No differences found.</p>
      </Show>

      <Show
        when={diffs() !== null && diffs()!.length! > 0 ? diffs() : undefined}
      >
        {(entries) => {
          const totalAlloc = entries().reduce(
            (s: number, e: ClassDiff) => s + e.alloc_size,
            0,
          );
          const totalFreed = entries().reduce((s, e) => s + e.freed_size, 0);
          const totalDelta = totalAlloc - totalFreed;

          return (
            <>
              <div
                style={{
                  'margin-bottom': '8px',
                  'font-size': '12px',
                  color: '#888',
                }}
              >
                {entries().length} constructors changed,{' '}
                <span
                  style={{
                    color:
                      totalDelta > 0
                        ? '#ef4444'
                        : totalDelta < 0
                          ? '#10b981'
                          : undefined,
                  }}
                >
                  {formatSigned(totalDelta)} net
                </span>
              </div>
              <table
                style={{
                  'border-collapse': 'collapse',
                  width: '100%',
                  'font-size': '13px',
                  'table-layout': 'fixed',
                }}
              >
                <colgroup>
                  <col />
                  <col style={{ width: '80px' }} />
                  <col style={{ width: '80px' }} />
                  <col style={{ width: '80px' }} />
                  <col style={{ width: '90px' }} />
                  <col style={{ width: '90px' }} />
                  <col style={{ width: '90px' }} />
                </colgroup>
                <thead>
                  <tr
                    style={{
                      'text-align': 'left',
                      'border-bottom': '1px solid #ccc',
                    }}
                  >
                    <th style={{ padding: '4px 8px' }}>Constructor</th>
                    <th style={{ ...numTd, 'font-weight': 'bold' }}># New</th>
                    <th style={{ ...numTd, 'font-weight': 'bold' }}>
                      # Deleted
                    </th>
                    <th style={{ ...numTd, 'font-weight': 'bold' }}># Delta</th>
                    <th style={{ ...numTd, 'font-weight': 'bold' }}>
                      Alloc Size
                    </th>
                    <th style={{ ...numTd, 'font-weight': 'bold' }}>
                      Freed Size
                    </th>
                    <th style={{ ...numTd, 'font-weight': 'bold' }}>
                      Size Delta
                    </th>
                  </tr>
                </thead>
                <tbody>
                  <For each={entries()}>
                    {(entry) => (
                      <tr>
                        <td
                          style={{
                            padding: '3px 8px',
                            overflow: 'hidden',
                            'text-overflow': 'ellipsis',
                            'white-space': 'nowrap',
                            'max-width': '0',
                          }}
                        >
                          {entry.name}
                        </td>
                        <td style={{ ...numTd, color: '#ef4444' }}>
                          {entry.new_count > 0
                            ? `+${entry.new_count.toLocaleString()}`
                            : ''}
                        </td>
                        <td style={{ ...numTd, color: '#10b981' }}>
                          {entry.deleted_count > 0
                            ? `-${entry.deleted_count.toLocaleString()}`
                            : ''}
                        </td>
                        <td
                          style={{
                            ...numTd,
                            color:
                              entry.delta_count > 0
                                ? '#ef4444'
                                : entry.delta_count < 0
                                  ? '#10b981'
                                  : undefined,
                            'font-weight': '600',
                          }}
                        >
                          {formatSignedCount(entry.delta_count)}
                        </td>
                        <td style={{ ...numTd, color: '#ef4444' }}>
                          {entry.alloc_size > 0
                            ? `+${formatBytes(entry.alloc_size)}`
                            : ''}
                        </td>
                        <td style={{ ...numTd, color: '#10b981' }}>
                          {entry.freed_size > 0
                            ? `-${formatBytes(entry.freed_size)}`
                            : ''}
                        </td>
                        <td
                          style={{
                            ...numTd,
                            color:
                              entry.size_delta > 0
                                ? '#ef4444'
                                : entry.size_delta < 0
                                  ? '#10b981'
                                  : undefined,
                            'font-weight': '600',
                          }}
                        >
                          {formatSigned(entry.size_delta)}
                        </td>
                      </tr>
                    )}
                  </For>
                </tbody>
              </table>
            </>
          );
        }}
      </Show>

      <Show when={!diffs() && !loading() && otherSnapshots().length === 0}>
        <p style={{ color: '#888' }}>
          Load another snapshot to compare against.
        </p>
      </Show>
    </div>
  );
}
