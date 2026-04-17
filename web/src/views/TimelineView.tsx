import { createSignal, createResource, Show, type JSX } from 'solid-js';
import type {
  TimelineInterval,
  AggregateEntry,
  ReachableSizeInfo,
} from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import type { NavigateOptions, EdgeInfo } from '../components/ObjectLink.tsx';
import { formatBytes } from '../components/format.ts';
import { ContextMenu } from '../components/ContextMenu.tsx';
import { SummaryTable } from './SummaryView.tsx';

export function TimelineView(props: {
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number, edgeInfo?: EdgeInfo) => void;
  reachableSizes: Map<number, ReachableSizeInfo>;
  reachablePending: Set<number>;
}): JSX.Element {
  const [intervals] = createResource(() =>
    props.call<TimelineInterval[]>({ type: 'getTimeline' }),
  );
  const [menu, setMenu] = createSignal<{
    x: number;
    y: number;
    intervalIndex: number;
  } | null>(null);
  const [selectedInterval, setSelectedInterval] = createSignal<number | null>(
    null,
  );
  const [intervalEntries, setIntervalEntries] = createSignal<
    AggregateEntry[] | null
  >(null);

  const loadInterval = async (index: number) => {
    setSelectedInterval(index);
    setIntervalEntries(null);
    const result = await props.call<AggregateEntry[]>({
      type: 'getSummaryForInterval',
      intervalIndex: index,
    });
    setIntervalEntries(result);
  };

  return (
    <Show when={intervals()} fallback={<p>Loading...</p>}>
      {(data) => {
        const items = data();
        if (items.length === 0) {
          return (
            <p style={{ color: '#888' }}>
              No allocation timeline data in this snapshot.
            </p>
          );
        }

        const maxSize = Math.max(...items.map((i) => i.size), 1);
        const totalCount = items.reduce((s, i) => s + i.count, 0);
        const totalSize = items.reduce((s, i) => s + i.size, 0);

        return (
          <div>
            <div style={{ 'max-width': '900px' }}>
              <p style={{ 'font-size': '14px', 'margin-bottom': '12px' }}>
                {items.length} intervals, {totalCount.toLocaleString()} live
                objects, {formatBytes(totalSize)} total
              </p>
              <div
                style={{
                  display: 'flex',
                  'flex-direction': 'column',
                  gap: '1px',
                }}
              >
                {items.map((interval, index) => {
                  const pct = maxSize > 0 ? (interval.size / maxSize) * 100 : 0;
                  const tsSec = interval.timestamp_us / 1_000_000;
                  const isSelected = () => selectedInterval() === index;
                  return (
                    <div
                      onContextMenu={(e) => {
                        e.preventDefault();
                        setMenu({
                          x: e.clientX,
                          y: e.clientY,
                          intervalIndex: index,
                        });
                      }}
                      onClick={() => loadInterval(index)}
                      style={{
                        display: 'flex',
                        'align-items': 'center',
                        'font-size': '12px',
                        height: '18px',
                        cursor: 'pointer',
                        'user-select': 'none',
                        background: isSelected() ? '#e8f0fe' : undefined,
                        'border-radius': '2px',
                      }}
                    >
                      <span
                        style={{
                          width: '60px',
                          'text-align': 'right',
                          'margin-right': '8px',
                          color: '#888',
                          'font-variant-numeric': 'tabular-nums',
                        }}
                      >
                        {tsSec.toFixed(1)}s
                      </span>
                      <span
                        style={{
                          width: '70px',
                          'text-align': 'right',
                          'margin-right': '8px',
                          'font-variant-numeric': 'tabular-nums',
                        }}
                      >
                        {formatBytes(interval.size)}
                      </span>
                      <div
                        style={{
                          flex: '1',
                          height: '12px',
                          background: '#f0f0f0',
                          'border-radius': '2px',
                          overflow: 'hidden',
                        }}
                      >
                        <div
                          style={{
                            width: `${pct}%`,
                            height: '100%',
                            'background-color':
                              interval.count === 0 ? '#ccc' : '#3b82f6',
                            'border-radius': '2px',
                          }}
                          title={`${interval.count} objects, ${formatBytes(interval.size)}`}
                        />
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>

            <Show when={selectedInterval() !== null && intervalEntries()}>
              {(entries) => {
                const idx = selectedInterval()!;
                const interval = items[idx];
                const tsSec = interval.timestamp_us / 1_000_000;
                return (
                  <div style={{ 'margin-top': '24px' }}>
                    <h3
                      style={{
                        'font-size': '14px',
                        margin: '0 0 8px',
                      }}
                    >
                      Live objects from interval {idx + 1} ({tsSec.toFixed(1)}s)
                      — {interval.count.toLocaleString()} objects,{' '}
                      {formatBytes(interval.size)}
                    </h3>
                    <SummaryTable
                      entries={entries()}
                      call={props.call}
                      objectsMessageType="getTimelineObjects"
                      onNavigate={props.onNavigate}
                      onContextMenu={props.onContextMenu}
                      reachableSizes={props.reachableSizes}
                      reachablePending={props.reachablePending}
                    />
                  </div>
                );
              }}
            </Show>

            <Show when={menu()}>
              {(m) => (
                <ContextMenu
                  x={m().x}
                  y={m().y}
                  onClose={() => setMenu(null)}
                  items={[
                    {
                      label: 'Show live objects',
                      action: () => loadInterval(m().intervalIndex),
                    },
                  ]}
                />
              )}
            </Show>
          </div>
        );
      }}
    </Show>
  );
}
