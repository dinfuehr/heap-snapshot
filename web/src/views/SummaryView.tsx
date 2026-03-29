import {
  createSignal,
  createResource,
  createMemo,
  Show,
  For,
  type JSX,
} from 'solid-js';
import type { AggregateEntry, SummaryExpanded } from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import type { NavigateOptions } from '../components/ObjectLink.tsx';
import { ObjectLink } from '../components/ObjectLink.tsx';
import { formatBytes } from '../components/format.ts';
import { TreeTablePager } from '../components/TreeTablePager.tsx';

const numTd = {
  padding: '2px 8px',
  'text-align': 'right' as const,
  'font-variant-numeric': 'tabular-nums',
};

export function SummaryView(props: {
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
  highlightNodeId: number | null;
}): JSX.Element {
  const [entries] = createResource(() =>
    props.call<AggregateEntry[]>({ type: 'getSummary' }),
  );
  const [expanded, setExpanded] = createSignal<string | null>(null);
  const [objects, setObjects] = createSignal<SummaryExpanded | null>(null);
  const [objOffset, setObjOffset] = createSignal(0);
  const [filter, setFilter] = createSignal('');

  const filtered = createMemo(() => {
    const e = entries();
    if (!e) return null;
    const f = filter().toLowerCase();
    if (!f) return e;
    return e.filter((entry) => entry.name.toLowerCase().includes(f));
  });

  const loadObjects = async (key: string, o: number, l: number) => {
    const result = await props.call<SummaryExpanded>({
      type: 'getSummaryObjects',
      constructor: key,
      offset: o,
      limit: l,
    });
    setObjects(result);
    setObjOffset(o);
  };

  const toggleExpand = async (key: string) => {
    if (expanded() === key) {
      setExpanded(null);
      setObjects(null);
      return;
    }
    setExpanded(key);
    await loadObjects(key, 0, 100);
  };

  return (
    <Show when={filtered()} fallback={<p>Loading...</p>}>
      {(list) => (
        <div>
          <div
            style={{
              'margin-bottom': '8px',
              display: 'flex',
              'align-items': 'center',
              gap: '8px',
            }}
          >
            <input
              type="text"
              value={filter()}
              onInput={(e) => setFilter(e.currentTarget.value)}
              placeholder="Filter constructors..."
              style={{
                padding: '4px 8px',
                'font-size': '13px',
                width: '250px',
              }}
            />
            <Show when={filter()}>
              <span style={{ 'font-size': '12px', color: '#888' }}>
                {list().length} of {entries()!.length} groups
              </span>
            </Show>
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
              <col style={{ width: '100px' }} />
              <col style={{ width: '110px' }} />
              <col style={{ width: '120px' }} />
              <col style={{ width: '75px' }} />
            </colgroup>
            <thead>
              <tr
                style={{
                  'text-align': 'left',
                  'border-bottom': '1px solid #ccc',
                }}
              >
                <th style={{ padding: '4px 8px' }}>Constructor</th>
                <th
                  style={{
                    padding: '4px 8px',
                    'text-align': 'right',
                    'white-space': 'nowrap',
                  }}
                >
                  Count
                </th>
                <th
                  style={{
                    padding: '4px 8px',
                    'text-align': 'right',
                    'white-space': 'nowrap',
                  }}
                >
                  Shallow Size
                </th>
                <th
                  style={{
                    padding: '4px 8px',
                    'text-align': 'right',
                    'white-space': 'nowrap',
                  }}
                >
                  Retained Size
                </th>
                <th
                  style={{
                    padding: '4px 8px',
                    'text-align': 'right',
                    'white-space': 'nowrap',
                  }}
                >
                  Reachable Size
                </th>
                <th
                  style={{
                    padding: '4px 8px',
                    'text-align': 'right',
                    'white-space': 'nowrap',
                  }}
                >
                  Status
                </th>
              </tr>
            </thead>
            <tbody>
              <For each={list()}>
                {(entry) => (
                  <>
                    <tr
                      onClick={() => toggleExpand(entry.key)}
                      style={{
                        cursor: 'pointer',
                        background:
                          expanded() === entry.key ? '#f0f0f0' : undefined,
                      }}
                    >
                      <td
                        style={{
                          padding: '2px 8px',
                          overflow: 'hidden',
                          'text-overflow': 'ellipsis',
                          'white-space': 'nowrap',
                          'max-width': '0',
                        }}
                      >
                        {expanded() === entry.key ? '\u25bc' : '\u25b6'}{' '}
                        {entry.name}
                      </td>
                      <td style={numTd}>{entry.count.toLocaleString()}</td>
                      <td style={numTd}>{formatBytes(entry.self_size)}</td>
                      <td style={numTd}>{formatBytes(entry.retained_size)}</td>
                      <td style={{ ...numTd, color: '#ccc' }}>{'\u2014'}</td>
                      <td />
                    </tr>
                    <Show when={expanded() === entry.key && objects()}>
                      {(objs) => (
                        <tr>
                          <td colSpan={6} style={{ padding: '4px 24px' }}>
                            <table
                              style={{
                                'border-collapse': 'collapse',
                                width: '100%',
                                'font-size': '12px',
                                'table-layout': 'fixed',
                              }}
                            >
                              <colgroup>
                                <col />
                                <col style={{ width: '90px' }} />
                                <col style={{ width: '100px' }} />
                                <col style={{ width: '120px' }} />
                                <col style={{ width: '75px' }} />
                              </colgroup>
                              <tbody>
                                <For each={objs().objects}>
                                  {(obj) => (
                                    <tr>
                                      <td
                                        style={{
                                          padding: '1px 8px',
                                          overflow: 'hidden',
                                          'text-overflow': 'ellipsis',
                                          'white-space': 'nowrap',
                                          'max-width': '0',
                                        }}
                                      >
                                        <ObjectLink
                                          nodeId={obj.id}
                                          onNavigate={props.onNavigate}
                                          onContextMenu={props.onContextMenu}
                                        />{' '}
                                        {obj.name}
                                      </td>
                                      <td style={numTd}>
                                        {formatBytes(obj.self_size)}
                                      </td>
                                      <td style={numTd}>
                                        {formatBytes(obj.retained_size)}
                                      </td>
                                      <td style={{ ...numTd, color: '#ccc' }}>
                                        {'\u2014'}
                                      </td>
                                      <td
                                        style={{
                                          ...numTd,
                                          color:
                                            obj.detachedness === 2
                                              ? '#ef4444'
                                              : obj.detachedness === 1
                                                ? '#10b981'
                                                : '#888',
                                          'font-weight':
                                            obj.detachedness === 2
                                              ? '600'
                                              : undefined,
                                        }}
                                      >
                                        {obj.detachedness === 2
                                          ? 'detached'
                                          : obj.detachedness === 1
                                            ? 'attached'
                                            : ''}
                                      </td>
                                    </tr>
                                  )}
                                </For>
                                <TreeTablePager
                                  depth={0}
                                  shown={objs().objects.length}
                                  total={objs().total}
                                  offset={objOffset()}
                                  filter=""
                                  onPageChange={(o, l) =>
                                    loadObjects(expanded()!, o, l)
                                  }
                                  onFilterChange={() => {}}
                                  onShowAll={() =>
                                    loadObjects(expanded()!, 0, 999999)
                                  }
                                />
                              </tbody>
                            </table>
                          </td>
                        </tr>
                      )}
                    </Show>
                  </>
                )}
              </For>
            </tbody>
          </table>
        </div>
      )}
    </Show>
  );
}
