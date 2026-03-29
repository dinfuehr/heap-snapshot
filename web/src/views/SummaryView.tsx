import { useEffect, useState, useMemo } from 'react';
import type {
  AggregateEntry,
  SummaryExpanded,
  ReachableSizeInfo,
} from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import type { NavigateOptions } from '../components/ObjectLink.tsx';
import { ObjectLink } from '../components/ObjectLink.tsx';
import { formatBytes } from '../components/format.ts';

interface Props {
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: React.MouseEvent, nodeId: number) => void;
  highlightNodeId: number | null;
  reachableSizes: Map<number, ReachableSizeInfo>;
}

export function SummaryView({
  call,
  onNavigate,
  onContextMenu,
  highlightNodeId,
  reachableSizes,
}: Props) {
  const [entries, setEntries] = useState<AggregateEntry[] | null>(null);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [objects, setObjects] = useState<SummaryExpanded | null>(null);
  const [filter, setFilter] = useState('');
  const [lastHighlight, setLastHighlight] = useState<number | null>(null);

  useEffect(() => {
    if (
      highlightNodeId !== null &&
      highlightNodeId !== lastHighlight &&
      entries
    ) {
      setLastHighlight(highlightNodeId);
      call<string>({ type: 'getConstructorForNode', nodeId: highlightNodeId })
        .then(async (key) => {
          setFilter('');
          setExpanded(key);
          const result = await call<SummaryExpanded>({
            type: 'getSummaryObjects',
            constructor: key,
            offset: 0,
            limit: 50,
          });
          setObjects(result);
        })
        .catch(() => {
          /* node not in any aggregate */
        });
    }
  }, [highlightNodeId, lastHighlight, entries, call]);

  useEffect(() => {
    if (!entries) {
      call<AggregateEntry[]>({ type: 'getSummary' }).then(setEntries);
    }
  }, [call, entries]);

  const filtered = useMemo(() => {
    if (!entries) return null;
    if (!filter) return entries;
    const lower = filter.toLowerCase();
    return entries.filter((e) => e.name.toLowerCase().includes(lower));
  }, [entries, filter]);

  const toggleExpand = async (key: string) => {
    if (expanded === key) {
      setExpanded(null);
      setObjects(null);
      return;
    }
    setExpanded(key);
    const result = await call<SummaryExpanded>({
      type: 'getSummaryObjects',
      constructor: key,
      offset: 0,
      limit: 50,
    });
    setObjects(result);
  };

  if (!filtered) return <p>Loading...</p>;

  return (
    <div>
      <div
        style={{
          marginBottom: 8,
          display: 'flex',
          alignItems: 'center',
          gap: 8,
        }}
      >
        <input
          type="text"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder="Filter constructors..."
          style={{ padding: '4px 8px', fontSize: 13, width: 250 }}
        />
        {filter && (
          <span style={{ fontSize: 12, color: '#888' }}>
            {filtered.length} of {entries!.length} groups
          </span>
        )}
      </div>
      <table
        style={{
          borderCollapse: 'collapse',
          width: '100%',
          fontSize: 13,
          tableLayout: 'fixed',
        }}
      >
        <colgroup>
          <col />
          <col style={{ width: 80 }} />
          <col style={{ width: 100 }} />
          <col style={{ width: 110 }} />
          <col style={{ width: 120 }} />
        </colgroup>
        <thead>
          <tr style={{ textAlign: 'left', borderBottom: '1px solid #ccc' }}>
            <th style={{ padding: '4px 8px' }}>Constructor</th>
            <th
              style={{
                padding: '4px 8px',
                textAlign: 'right',
                whiteSpace: 'nowrap',
              }}
            >
              Count
            </th>
            <th
              style={{
                padding: '4px 8px',
                textAlign: 'right',
                whiteSpace: 'nowrap',
              }}
            >
              Shallow Size
            </th>
            <th
              style={{
                padding: '4px 8px',
                textAlign: 'right',
                whiteSpace: 'nowrap',
              }}
            >
              Retained Size
            </th>
            <th
              style={{
                padding: '4px 8px',
                textAlign: 'right',
                whiteSpace: 'nowrap',
              }}
            >
              Reachable Size
            </th>
          </tr>
        </thead>
        <tbody>
          {filtered.map((entry) => (
            <>
              <tr
                key={entry.key}
                onClick={() => toggleExpand(entry.key)}
                style={{
                  cursor: 'pointer',
                  background: expanded === entry.key ? '#f0f0f0' : undefined,
                }}
              >
                <td
                  style={{
                    padding: '2px 8px',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    whiteSpace: 'nowrap',
                    maxWidth: 0,
                  }}
                >
                  {expanded === entry.key ? '\u25bc' : '\u25b6'} {entry.name}
                </td>
                <td
                  style={{
                    padding: '2px 8px',
                    textAlign: 'right',
                    fontVariantNumeric: 'tabular-nums',
                  }}
                >
                  {entry.count.toLocaleString()}
                </td>
                <td
                  style={{
                    padding: '2px 8px',
                    textAlign: 'right',
                    fontVariantNumeric: 'tabular-nums',
                  }}
                >
                  {formatBytes(entry.self_size)}
                </td>
                <td
                  style={{
                    padding: '2px 8px',
                    textAlign: 'right',
                    fontVariantNumeric: 'tabular-nums',
                  }}
                >
                  {formatBytes(entry.retained_size)}
                </td>
                <td
                  style={{
                    padding: '2px 8px',
                    textAlign: 'right',
                    fontVariantNumeric: 'tabular-nums',
                    color: '#ccc',
                  }}
                >
                  {'\u2014'}
                </td>
              </tr>
              {expanded === entry.key && objects && (
                <tr key={entry.key + '-expanded'}>
                  <td colSpan={5} style={{ padding: '4px 24px' }}>
                    <table
                      style={{
                        borderCollapse: 'collapse',
                        width: '100%',
                        fontSize: 12,
                        tableLayout: 'fixed',
                      }}
                    >
                      <colgroup>
                        <col />
                        <col style={{ width: 90 }} />
                        <col style={{ width: 100 }} />
                        <col style={{ width: 120 }} />
                      </colgroup>
                      <tbody>
                        {objects.objects.map((obj) => (
                          <tr key={obj.id}>
                            <td
                              style={{
                                padding: '1px 8px',
                                overflow: 'hidden',
                                textOverflow: 'ellipsis',
                                whiteSpace: 'nowrap',
                                maxWidth: 0,
                              }}
                            >
                              <ObjectLink
                                nodeId={obj.id}
                                onNavigate={onNavigate}
                                onContextMenu={onContextMenu}
                              />{' '}
                              {obj.name}
                            </td>
                            <td
                              style={{
                                padding: '1px 8px',
                                textAlign: 'right',
                                fontVariantNumeric: 'tabular-nums',
                              }}
                            >
                              {formatBytes(obj.self_size)}
                            </td>
                            <td
                              style={{
                                padding: '1px 8px',
                                textAlign: 'right',
                                fontVariantNumeric: 'tabular-nums',
                              }}
                            >
                              {formatBytes(obj.retained_size)}
                            </td>
                            <td
                              style={{
                                padding: '1px 8px',
                                textAlign: 'right',
                                fontVariantNumeric: 'tabular-nums',
                                color: reachableSizes.get(obj.id)
                                  ? undefined
                                  : '#ccc',
                              }}
                              title={
                                reachableSizes.get(obj.id)?.native_contexts
                                  .length
                                  ? reachableSizes
                                      .get(obj.id)!
                                      .native_contexts.map(
                                        (c) => `${c.label} (${c.detachedness})`,
                                      )
                                      .join('\n')
                                  : undefined
                              }
                            >
                              {reachableSizes.get(obj.id)
                                ? formatBytes(reachableSizes.get(obj.id)!.size)
                                : '\u2014'}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                    {objects.total > objects.objects.length && (
                      <p style={{ fontSize: 11, color: '#888' }}>
                        Showing {objects.objects.length} of {objects.total}{' '}
                        objects
                      </p>
                    )}
                  </td>
                </tr>
              )}
            </>
          ))}
        </tbody>
      </table>
    </div>
  );
}
