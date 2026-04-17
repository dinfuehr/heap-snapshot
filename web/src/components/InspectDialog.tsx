import { onCleanup, onMount, type JSX } from 'solid-js';
import type { NodeInfo } from '../types.ts';
import type { EdgeInfo } from './ObjectLink.tsx';
import { formatBytes } from './format.ts';

function detachednessLabel(d: number): string {
  if (d === 1) return 'Attached';
  if (d === 2) return 'Detached';
  return 'Unknown';
}

export function InspectDialog(props: {
  info: NodeInfo;
  edgeInfo?: EdgeInfo;
  onClose: () => void;
}): JSX.Element {
  let ref: HTMLDivElement | undefined;

  onMount(() => {
    const handler = (e: MouseEvent) => {
      if (ref && !ref.contains(e.target as Node)) {
        props.onClose();
      }
    };
    const keyHandler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') props.onClose();
    };
    document.addEventListener('mousedown', handler);
    document.addEventListener('keydown', keyHandler);
    onCleanup(() => {
      document.removeEventListener('mousedown', handler);
      document.removeEventListener('keydown', keyHandler);
    });
  });

  const n = props.info;
  const rows: [string, string][] = [
    ['ID', `@${n.id}`],
    ['Ordinal', `${n.ordinal}`],
    ['Type', n.node_type],
    ['Name', n.name],
    ['Class', n.class_name],
    ['Self size', `${formatBytes(n.self_size)} (${n.self_size})`],
    ['Retained size', `${formatBytes(n.retained_size)} (${n.retained_size})`],
    ['Distance', `${n.distance}`],
    ['Detachedness', detachednessLabel(n.detachedness)],
    ['Edge count', `${n.edge_count}`],
  ];
  if (n.ctx) {
    rows.push(['Context', n.ctx_label || n.ctx]);
  }

  const edgeRows: [string, string][] = [];
  if (props.edgeInfo) {
    edgeRows.push(['Type', props.edgeInfo.edgeType]);
    edgeRows.push(['Name', props.edgeInfo.edgeName]);
    edgeRows.push(['Parent', `@${props.edgeInfo.parentId}`]);
  }

  return (
    <div
      ref={ref}
      style={{
        position: 'fixed',
        top: '50%',
        left: '50%',
        transform: 'translate(-50%, -50%)',
        background: '#fff',
        border: '1px solid #ccc',
        'border-radius': '6px',
        'box-shadow': '0 4px 16px rgba(0,0,0,0.2)',
        'z-index': 1000,
        padding: '16px',
        'min-width': '320px',
        'font-size': '13px',
      }}
    >
      <div
        style={{
          'font-weight': '600',
          'margin-bottom': '12px',
          'font-size': '14px',
        }}
      >
        Inspect Node
      </div>
      <table style={{ 'border-collapse': 'collapse', width: '100%' }}>
        <tbody>
          {rows.map(([label, value]) => (
            <tr>
              <td
                style={{
                  padding: '2px 12px 2px 0',
                  color: '#666',
                  'white-space': 'nowrap',
                  'vertical-align': 'top',
                }}
              >
                {label}
              </td>
              <td
                style={{
                  padding: '2px 0',
                  'word-break': 'break-all',
                }}
              >
                {value}
              </td>
            </tr>
          ))}
          {edgeRows.length > 0 && (
            <>
              <tr>
                <td
                  colSpan={2}
                  style={{
                    'padding-top': '10px',
                    'font-weight': '600',
                    'font-size': '14px',
                  }}
                >
                  Edge (from parent)
                </td>
              </tr>
              {edgeRows.map(([label, value]) => (
                <tr>
                  <td
                    style={{
                      padding: '2px 12px 2px 0',
                      color: '#666',
                      'white-space': 'nowrap',
                      'vertical-align': 'top',
                    }}
                  >
                    {label}
                  </td>
                  <td
                    style={{
                      padding: '2px 0',
                      'word-break': 'break-all',
                    }}
                  >
                    {value}
                  </td>
                </tr>
              ))}
            </>
          )}
        </tbody>
      </table>
    </div>
  );
}
