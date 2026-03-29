import { useEffect, useState } from 'react';
import type { NativeContext } from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import type { NavigateOptions } from '../components/ObjectLink.tsx';
import { ObjectLink } from '../components/ObjectLink.tsx';
import { formatBytes } from '../components/format.ts';

interface Props {
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: React.MouseEvent, nodeId: number) => void;
}

export function ContextsView({ call, onNavigate, onContextMenu }: Props) {
  const [contexts, setContexts] = useState<NativeContext[] | null>(null);

  useEffect(() => {
    if (!contexts) {
      call<NativeContext[]>({ type: 'getNativeContexts' }).then(setContexts);
    }
  }, [call, contexts]);

  if (!contexts) return <p>Loading...</p>;

  if (contexts.length === 0) return <p>No native contexts found.</p>;

  return (
    <div>
      <p style={{ fontSize: 13, color: '#888', margin: '0 0 8px' }}>
        {contexts.length} native context{contexts.length !== 1 ? 's' : ''}{' '}
        (JavaScript realms)
      </p>
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
          <col style={{ width: 90 }} />
          <col style={{ width: 100 }} />
          <col style={{ width: 110 }} />
        </colgroup>
        <thead>
          <tr style={{ textAlign: 'left', borderBottom: '1px solid #ccc' }}>
            <th style={{ padding: '4px 8px' }}>Context</th>
            <th
              style={{
                padding: '4px 8px',
                textAlign: 'right',
                whiteSpace: 'nowrap',
              }}
            >
              Status
            </th>
            <th
              style={{
                padding: '4px 8px',
                textAlign: 'right',
                whiteSpace: 'nowrap',
              }}
            >
              Self Size
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
          </tr>
        </thead>
        <tbody>
          {contexts.map((ctx) => (
            <tr key={ctx.id}>
              <td
                style={{
                  padding: '2px 8px',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                  maxWidth: 0,
                }}
              >
                <ObjectLink
                  nodeId={ctx.id}
                  onNavigate={onNavigate}
                  onContextMenu={onContextMenu}
                />{' '}
                {ctx.label}
              </td>
              <td style={{ padding: '2px 8px', textAlign: 'right' }}>
                <span
                  style={{
                    color:
                      ctx.detachedness === 'detached'
                        ? '#ef4444'
                        : ctx.detachedness === 'attached'
                          ? '#10b981'
                          : '#888',
                    fontWeight:
                      ctx.detachedness === 'detached' ? 600 : undefined,
                  }}
                >
                  {ctx.detachedness}
                </span>
              </td>
              <td
                style={{
                  padding: '2px 8px',
                  textAlign: 'right',
                  fontVariantNumeric: 'tabular-nums',
                }}
              >
                {formatBytes(ctx.self_size)}
              </td>
              <td
                style={{
                  padding: '2px 8px',
                  textAlign: 'right',
                  fontVariantNumeric: 'tabular-nums',
                }}
              >
                {formatBytes(ctx.retained_size)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
