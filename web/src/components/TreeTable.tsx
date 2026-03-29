import { useCallback, useId, type ReactNode } from 'react';
import { formatBytes } from './format.ts';
import { ObjectLink, type NavigateOptions } from './ObjectLink.tsx';
import { useSelection, useReachableSizes } from './SelectionContext.tsx';

const numStyle: React.CSSProperties = {
  padding: '2px 8px',
  textAlign: 'right',
  fontVariantNumeric: 'tabular-nums',
  whiteSpace: 'nowrap',
};

export interface RowSelection {
  rowId: string;
  nodeId: number;
}

export function TreeTableHeader() {
  return (
    <thead>
      <tr style={{ textAlign: 'left', borderBottom: '1px solid #ccc' }}>
        <th style={{ padding: '4px 8px', width: '100%' }}>Object</th>
        <th
          style={{
            padding: '4px 8px',
            textAlign: 'right',
            whiteSpace: 'nowrap',
          }}
        >
          Distance
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
        <th
          style={{
            padding: '4px 8px',
            textAlign: 'right',
            whiteSpace: 'nowrap',
          }}
        >
          Reachable Size
        </th>
        <th
          style={{
            padding: '4px 8px',
            textAlign: 'right',
            whiteSpace: 'nowrap',
          }}
        >
          Status
        </th>
      </tr>
    </thead>
  );
}

export function TreeTableRow({
  depth,
  expanded,
  onToggle,
  label,
  linkId,
  onNavigate,
  onContextMenu,
  detachedness,
  distance,
  selfSize,
  retainedSize,
  children,
}: {
  depth: number;
  expanded?: boolean;
  onToggle?: () => void;
  label: ReactNode;
  linkId?: number;
  onNavigate?: (opts: NavigateOptions) => void;
  onContextMenu?: (e: React.MouseEvent, nodeId: number) => void;
  detachedness?: number;
  distance?: number;
  selfSize: number;
  retainedSize: number;
  children?: ReactNode;
}) {
  const rowId = useId();
  const indent = depth * 16;
  const expandable = onToggle !== undefined;
  const { selection, onSelect } = useSelection();
  const reachableSizes = useReachableSizes();
  const reachableInfo =
    linkId !== undefined ? reachableSizes.get(linkId) : undefined;

  const select = useCallback(() => {
    if (linkId !== undefined) {
      onSelect({ rowId, nodeId: linkId });
    }
  }, [linkId, onSelect, rowId]);

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      if ((e.target as HTMLElement).closest('a')) return;
      if (expandable) onToggle!();
      select();
    },
    [expandable, onToggle, select],
  );

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      if (linkId !== undefined && onContextMenu) {
        e.preventDefault();
        select();
        onContextMenu(e, linkId);
      }
    },
    [linkId, onContextMenu, select],
  );

  let bg: string | undefined;
  if (selection) {
    if (selection.rowId === rowId) {
      bg = '#e8f0fe';
    } else if (linkId !== undefined && selection.nodeId === linkId) {
      bg = '#fef9e7';
    }
  }

  return (
    <>
      <tr
        onClick={handleClick}
        onContextMenu={handleContextMenu}
        style={{ cursor: 'pointer', background: bg }}
      >
        <td
          style={{
            padding: '2px 8px',
            paddingLeft: 8 + indent,
            maxWidth: 0,
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {expandable && (
            <span style={{ display: 'inline-block', width: 16 }}>
              {expanded ? '\u25bc' : '\u25b6'}
            </span>
          )}
          {!expandable && (
            <span style={{ display: 'inline-block', width: 16 }}> </span>
          )}
          {linkId !== undefined && onNavigate && onContextMenu ? (
            <ObjectLink
              nodeId={linkId}
              onNavigate={onNavigate}
              onContextMenu={onContextMenu}
            />
          ) : null}
          {linkId !== undefined ? ' ' : null}
          {label}
        </td>
        <td style={numStyle}>{distance !== undefined ? distance : ''}</td>
        <td style={numStyle}>{formatBytes(selfSize)}</td>
        <td style={numStyle}>{formatBytes(retainedSize)}</td>
        <td
          style={{
            ...numStyle,
            color: reachableInfo !== undefined ? undefined : '#ccc',
          }}
          title={
            reachableInfo?.native_contexts.length
              ? reachableInfo.native_contexts
                  .map((c) => `${c.label} (${c.detachedness})`)
                  .join('\n')
              : undefined
          }
        >
          {reachableInfo !== undefined
            ? formatBytes(reachableInfo.size)
            : '\u2014'}
        </td>
        <td
          style={{
            ...numStyle,
            color:
              detachedness === 2
                ? '#ef4444'
                : detachedness === 1
                  ? '#10b981'
                  : '#888',
            fontWeight: detachedness === 2 ? 600 : undefined,
          }}
        >
          {detachedness === 2
            ? 'detached'
            : detachedness === 1
              ? 'attached'
              : ''}
        </td>
      </tr>
      {children}
    </>
  );
}

export function TreeTableShell({ children }: { children: ReactNode }) {
  return (
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
        <col style={{ width: 70 }} />
        <col style={{ width: 90 }} />
        <col style={{ width: 100 }} />
        <col style={{ width: 110 }} />
        <col style={{ width: 75 }} />
      </colgroup>
      <TreeTableHeader />
      <tbody>{children}</tbody>
    </table>
  );
}

export function TreeTableMore({
  depth,
  shown,
  total,
  label,
}: {
  depth: number;
  shown: number;
  total: number;
  label?: string;
}) {
  if (shown >= total) return null;
  return (
    <tr>
      <td
        colSpan={6}
        style={{
          padding: '2px 8px',
          paddingLeft: 8 + depth * 16,
          color: '#888',
          fontSize: 12,
        }}
      >
        ({shown} of {total} {label ?? 'children'} shown)
      </td>
    </tr>
  );
}
