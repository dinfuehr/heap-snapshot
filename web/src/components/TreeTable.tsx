import type { JSX } from 'solid-js';
import { formatBytes } from './format.ts';
import { ObjectLink, type NavigateOptions, type EdgeInfo } from './ObjectLink.tsx';
import type { ReachableSizeInfo } from '../types.ts';

const numStyle = {
  padding: '2px 8px',
  'text-align': 'right' as const,
  'font-variant-numeric': 'tabular-nums',
  'white-space': 'nowrap' as const,
};

export interface RowSelection {
  rowId: number;
  nodeId: number;
}

let nextRowId = 0;

export function TreeTableRow(props: {
  depth: number;
  expanded?: boolean;
  loading?: boolean;
  onToggle?: () => void;
  prefix?: JSX.Element;
  label: JSX.Element;
  linkId?: number;
  onNavigate?: (opts: NavigateOptions) => void;
  onContextMenu?: (e: MouseEvent, nodeId: number, edgeInfo?: EdgeInfo) => void;
  selection?: RowSelection | null;
  onSelect?: (sel: RowSelection) => void;
  detachedness?: number;
  ctx?: string;
  ctxLabel?: string;
  distance?: number;
  selfSize: number;
  retainedSize: number;
  reachableInfo?: ReachableSizeInfo;
  reachableLoading?: boolean;
  children?: JSX.Element;
}): JSX.Element {
  const rowId = nextRowId++;
  const indent = () => props.depth * 16;

  const handleClick = (e: MouseEvent) => {
    if ((e.target as HTMLElement).closest('a')) return;
    if (props.linkId !== undefined && props.onSelect) {
      props.onSelect({ rowId, nodeId: props.linkId });
    }
  };

  const handleDblClick = (e: MouseEvent) => {
    if ((e.target as HTMLElement).closest('a')) return;
    props.onToggle?.();
  };

  const handleContextMenu = (e: MouseEvent) => {
    if (props.linkId !== undefined && props.onContextMenu) {
      e.preventDefault();
      if (props.onSelect) {
        props.onSelect({ rowId, nodeId: props.linkId });
      }
      props.onContextMenu(e, props.linkId);
    }
  };

  const bg = () => {
    const sel = props.selection;
    if (!sel) return undefined;
    if (sel.rowId === rowId) return '#e8f0fe';
    if (props.linkId !== undefined && sel.nodeId === props.linkId)
      return '#fef9e7';
    return undefined;
  };

  const ri = () => props.reachableInfo;

  return (
    <>
      <tr
        data-node-id={props.linkId}
        onClick={handleClick}
        onDblClick={handleDblClick}
        onContextMenu={handleContextMenu}
        style={{ cursor: 'pointer', background: bg() }}
      >
        <td
          style={{
            padding: '2px 8px',
            'padding-left': `${8 + indent()}px`,
            'white-space': 'nowrap',
            overflow: 'hidden',
            'text-overflow': 'ellipsis',
          }}
        >
          {props.onToggle !== undefined ? (
            <span
              style={{
                display: 'inline-block',
                width: '16px',
                cursor: 'pointer',
                'user-select': 'none',
              }}
              onClick={(e) => {
                e.stopPropagation();
                props.onToggle?.();
              }}
            >
              {props.loading ? '\u22EF' : props.expanded ? '\u25bc' : '\u25b6'}
            </span>
          ) : (
            <span style={{ display: 'inline-block', width: '16px' }}> </span>
          )}
          {props.prefix}
          {props.linkId !== undefined &&
          props.onNavigate &&
          props.onContextMenu ? (
            <ObjectLink
              nodeId={props.linkId}
              onNavigate={props.onNavigate}
              onContextMenu={props.onContextMenu}
            />
          ) : null}
          {props.linkId !== undefined ? ' ' : null}
          {props.label}
        </td>
        <td style={numStyle}>
          {props.distance !== undefined ? props.distance : ''}
        </td>
        <td style={numStyle}>{formatBytes(props.selfSize)}</td>
        <td style={numStyle}>{formatBytes(props.retainedSize)}</td>
        <td
          style={{
            ...numStyle,
            color: ri() !== undefined ? undefined : '#ccc',
          }}
          title={
            ri()?.native_contexts.length
              ? ri()!
                  .native_contexts.map((c) => `${c.label} (${c.detachedness})`)
                  .join('\n')
              : undefined
          }
        >
          {ri() !== undefined
            ? formatBytes(ri()!.size)
            : props.reachableLoading
              ? '\u22EF'
              : '\u2014'}
        </td>
        <td
          style={{
            ...numStyle,
            color:
              props.detachedness === 2
                ? '#ef4444'
                : props.detachedness === 1
                  ? '#10b981'
                  : '#888',
            'font-weight': props.detachedness === 2 ? '600' : undefined,
          }}
        >
          {props.detachedness === 2
            ? 'detached'
            : props.detachedness === 1
              ? 'attached'
              : ''}
        </td>
        <td style={numStyle} title={props.ctxLabel || undefined}>
          {props.ctx ?? ''}
        </td>
      </tr>
      {props.children}
    </>
  );
}

export function TreeTableShell(props: { children: JSX.Element }): JSX.Element {
  return (
    <div
      style={{
        flex: '1',
        'min-height': '0',
        overflow: 'auto',
      }}
    >
      <table
        style={{
          'border-collapse': 'collapse',
          width: '100%',
          'table-layout': 'fixed',
          'font-size': '13px',
        }}
      >
        <colgroup>
          <col />
          <col style={{ width: '80px' }} />
          <col style={{ width: '90px' }} />
          <col style={{ width: '110px' }} />
          <col style={{ width: '120px' }} />
          <col style={{ width: '80px' }} />
          <col style={{ width: '50px' }} />
        </colgroup>
        <thead>
          <tr
            style={{
              'text-align': 'left',
              'border-bottom': '1px solid #ccc',
              background: 'white',
              position: 'sticky',
              top: '0',
              'z-index': 1,
            }}
          >
            <th style={{ padding: '4px 8px', width: '100%' }}>Object</th>
            <th
              style={{
                padding: '4px 8px',
                'text-align': 'right',
                'white-space': 'nowrap',
              }}
            >
              Distance
            </th>
            <th
              style={{
                padding: '4px 8px',
                'text-align': 'right',
                'white-space': 'nowrap',
              }}
            >
              Self Size
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
            <th
              style={{
                padding: '4px 8px',
                'text-align': 'right',
                'white-space': 'nowrap',
              }}
            >
              Ctx
            </th>
          </tr>
        </thead>
        <tbody>{props.children}</tbody>
      </table>
    </div>
  );
}

export function TreeTableLoading(props: { depth: number }): JSX.Element {
  return (
    <tr>
      <td
        colSpan={7}
        style={{
          padding: '2px 8px',
          'padding-left': `${8 + props.depth * 16}px`,
          color: '#888',
          'font-size': '12px',
        }}
      >
        Loading...
      </td>
    </tr>
  );
}
