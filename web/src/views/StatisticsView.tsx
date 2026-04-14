import { createResource, Show, type JSX } from 'solid-js';
import type { Statistics } from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import { formatBytes } from '../components/format.ts';

const CATEGORIES: { key: keyof Statistics; label: string; color: string }[] = [
  { key: 'v8heap_total', label: 'V8 Heap', color: '#3b82f6' },
  { key: 'code', label: 'Code', color: '#8b5cf6' },
  { key: 'strings', label: 'Strings', color: '#f59e0b' },
  { key: 'js_arrays', label: 'JS Arrays', color: '#10b981' },
  { key: 'system', label: 'System', color: '#6366f1' },
  { key: 'native_total', label: 'Native', color: '#ef4444' },
];

function pct(value: number, total: number): string {
  if (total === 0) return '0.0%';
  return ((value / total) * 100).toFixed(1) + '%';
}

export function StatisticsView(props: { call: SnapshotCall }): JSX.Element {
  const [stats] = createResource(() =>
    props.call<Statistics>({ type: 'getStatistics' }),
  );

  return (
    <div
      style={{
        'font-variant-numeric': 'tabular-nums',
        overflow: 'auto',
        flex: '1',
        'min-height': '0',
        padding: '0 16px',
      }}
    >
      {!stats() ? (
        <p>Loading...</p>
      ) : (
        <>
          <table
            style={{
              'border-collapse': 'collapse',
              width: '100%',
              'font-size': '14px',
            }}
          >
            <tbody>
              <tr style={{ 'border-bottom': '1px solid #ddd' }}>
                <td style={{ padding: '6px 0', 'font-weight': 'bold' }}>
                  Total
                </td>
                <td style={{ padding: '6px 12px', 'text-align': 'right' }}>
                  {formatBytes(stats()!.total)}
                </td>
                <td />
              </tr>
              {CATEGORIES.map(({ key, label, color }) => (
                <tr>
                  <td style={{ padding: '3px 0', color, 'font-weight': '600' }}>
                    {label}
                  </td>
                  <td style={{ padding: '3px 12px', 'text-align': 'right' }}>
                    {formatBytes(stats()![key] as number)}
                  </td>
                  <td style={{ padding: '3px 0', color: '#888' }}>
                    {pct(stats()![key] as number, stats()!.total)}
                  </td>
                </tr>
              ))}
              <tr style={{ 'border-top': '1px solid #ddd' }}>
                <td
                  style={{
                    padding: '3px 0',
                    'padding-left': '16px',
                    color: '#888',
                  }}
                >
                  Extra Native
                </td>
                <td style={{ padding: '3px 12px', 'text-align': 'right' }}>
                  {formatBytes(stats()!.extra_native_bytes)}
                </td>
                <td
                  style={{
                    padding: '3px 0',
                    color: '#888',
                    'font-size': '12px',
                  }}
                >
                  (subset of Native)
                </td>
              </tr>
              <tr>
                <td
                  style={{
                    padding: '3px 0',
                    'padding-left': '16px',
                    color: '#888',
                  }}
                >
                  Typed Arrays
                </td>
                <td style={{ padding: '3px 12px', 'text-align': 'right' }}>
                  {formatBytes(stats()!.typed_arrays)}
                </td>
                <td
                  style={{
                    padding: '3px 0',
                    color: '#888',
                    'font-size': '12px',
                  }}
                >
                  (subset of Native)
                </td>
              </tr>
              <tr style={{ 'border-top': '1px solid #ddd' }}>
                <td style={{ padding: '6px 0', color: '#888' }}>Unreachable</td>
                <td style={{ padding: '3px 12px', 'text-align': 'right' }}>
                  {formatBytes(stats()!.unreachable_size)}
                </td>
                <td
                  style={{
                    padding: '3px 0',
                    color: '#888',
                    'font-size': '12px',
                  }}
                >
                  ({stats()!.unreachable_count.toLocaleString()} objects)
                </td>
              </tr>
            </tbody>
          </table>
          <div
            style={{
              display: 'flex',
              height: '24px',
              'border-radius': '4px',
              overflow: 'hidden',
              'margin-top': '16px',
            }}
          >
            {CATEGORIES.map(({ key, label, color }) => {
              const w =
                stats()!.total > 0
                  ? ((stats()![key] as number) / stats()!.total) * 100
                  : 0;
              if (w < 0.3) return null;
              return (
                <div
                  title={`${label}: ${formatBytes(stats()![key] as number)} (${pct(stats()![key] as number, stats()!.total)})`}
                  style={{
                    width: `${w}%`,
                    'background-color': color,
                    'min-width': w > 0 ? '2px' : '0',
                  }}
                />
              );
            })}
          </div>
          <div
            style={{
              display: 'flex',
              gap: '16px',
              'margin-top': '8px',
              'font-size': '12px',
              'flex-wrap': 'wrap',
            }}
          >
            {CATEGORIES.map(({ label, color }) => (
              <div
                style={{ display: 'flex', 'align-items': 'center', gap: '4px' }}
              >
                <div
                  style={{
                    width: '10px',
                    height: '10px',
                    'background-color': color,
                    'border-radius': '2px',
                  }}
                />
                <span>{label}</span>
              </div>
            ))}
          </div>
          <ContextAttribution stats={stats()!} />
        </>
      )}
    </div>
  );
}

const CONTEXT_COLORS = [
  '#3b82f6',
  '#ef4444',
  '#10b981',
  '#f59e0b',
  '#8b5cf6',
  '#ec4899',
  '#14b8a6',
  '#f97316',
  '#6366f1',
  '#84cc16',
];

function ContextAttribution(props: { stats: Statistics }): JSX.Element {
  const ctxSizes = () => props.stats.context_sizes;
  const attrTotal = () =>
    ctxSizes().reduce((s, c) => s + c.size, 0) +
    props.stats.shared_size +
    props.stats.unattributed_size;

  return (
    <Show when={ctxSizes().length > 0}>
      <h3
        style={{
          'font-size': '14px',
          'font-weight': 'bold',
          'margin-top': '24px',
          'margin-bottom': '8px',
        }}
      >
        Native Context Attribution
      </h3>
      <table
        style={{
          'border-collapse': 'collapse',
          width: '100%',
          'font-size': '14px',
        }}
      >
        <tbody>
          {ctxSizes().map(({ label, size }, i) => (
            <tr>
              <td
                style={{
                  padding: '3px 0',
                  color: CONTEXT_COLORS[i % CONTEXT_COLORS.length],
                  'font-weight': '600',
                  'max-width': '300px',
                  overflow: 'hidden',
                  'text-overflow': 'ellipsis',
                  'white-space': 'nowrap',
                }}
                title={label}
              >
                {label}
              </td>
              <td style={{ padding: '3px 12px', 'text-align': 'right' }}>
                {formatBytes(size)}
              </td>
              <td style={{ padding: '3px 0', color: '#888' }}>
                {pct(size, attrTotal())}
              </td>
            </tr>
          ))}
          <tr style={{ 'border-top': '1px solid #ddd' }}>
            <td style={{ padding: '3px 0', color: '#888' }}>Shared</td>
            <td style={{ padding: '3px 12px', 'text-align': 'right' }}>
              {formatBytes(props.stats.shared_size)}
            </td>
            <td style={{ padding: '3px 0', color: '#888' }}>
              {pct(props.stats.shared_size, attrTotal())}
            </td>
          </tr>
          <tr>
            <td style={{ padding: '3px 0', color: '#888' }}>Unattributed</td>
            <td style={{ padding: '3px 12px', 'text-align': 'right' }}>
              {formatBytes(props.stats.unattributed_size)}
            </td>
            <td style={{ padding: '3px 0', color: '#888' }}>
              {pct(props.stats.unattributed_size, attrTotal())}
            </td>
          </tr>
        </tbody>
      </table>
      <div
        style={{
          display: 'flex',
          height: '24px',
          'border-radius': '4px',
          overflow: 'hidden',
          'margin-top': '12px',
        }}
      >
        {ctxSizes().map(({ size }, i) => {
          const w = attrTotal() > 0 ? (size / attrTotal()) * 100 : 0;
          if (w < 0.3) return null;
          return (
            <div
              style={{
                width: `${w}%`,
                'background-color': CONTEXT_COLORS[i % CONTEXT_COLORS.length],
                'min-width': '2px',
              }}
            />
          );
        })}
        {(() => {
          const w =
            attrTotal() > 0 ? (props.stats.shared_size / attrTotal()) * 100 : 0;
          if (w < 0.3) return null;
          return (
            <div
              style={{
                width: `${w}%`,
                'background-color': '#9ca3af',
                'min-width': '2px',
              }}
            />
          );
        })()}
        {(() => {
          const w =
            attrTotal() > 0
              ? (props.stats.unattributed_size / attrTotal()) * 100
              : 0;
          if (w < 0.3) return null;
          return (
            <div
              style={{
                width: `${w}%`,
                'background-color': '#d1d5db',
                'min-width': '2px',
              }}
            />
          );
        })()}
      </div>
      <div
        style={{
          display: 'flex',
          gap: '16px',
          'margin-top': '8px',
          'font-size': '12px',
          'flex-wrap': 'wrap',
        }}
      >
        {ctxSizes().map(({ label }, i) => (
          <div style={{ display: 'flex', 'align-items': 'center', gap: '4px' }}>
            <div
              style={{
                width: '10px',
                height: '10px',
                'background-color': CONTEXT_COLORS[i % CONTEXT_COLORS.length],
                'border-radius': '2px',
              }}
            />
            <span>{label}</span>
          </div>
        ))}
        <div style={{ display: 'flex', 'align-items': 'center', gap: '4px' }}>
          <div
            style={{
              width: '10px',
              height: '10px',
              'background-color': '#9ca3af',
              'border-radius': '2px',
            }}
          />
          <span>Shared</span>
        </div>
        <div style={{ display: 'flex', 'align-items': 'center', gap: '4px' }}>
          <div
            style={{
              width: '10px',
              height: '10px',
              'background-color': '#d1d5db',
              'border-radius': '2px',
            }}
          />
          <span>Unattributed</span>
        </div>
      </div>
    </Show>
  );
}
