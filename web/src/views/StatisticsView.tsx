import { createResource, type JSX } from 'solid-js';
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
      style={{ 'max-width': '600px', 'font-variant-numeric': 'tabular-nums' }}
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
                <td style={{ padding: '6px 0', color: '#888' }}>
                  Extra Native
                </td>
                <td style={{ padding: '6px 12px', 'text-align': 'right' }}>
                  {formatBytes(stats()!.extra_native_bytes)}
                </td>
                <td style={{ padding: '6px 0', color: '#888' }}>
                  {pct(stats()!.extra_native_bytes, stats()!.total)}{' '}
                  <span style={{ 'font-size': '12px' }}>
                    (subset of Native)
                  </span>
                </td>
              </tr>
              <tr style={{ 'border-top': '1px solid #ddd' }}>
                <td style={{ padding: '6px 0', color: '#888' }}>
                  Typed Arrays
                </td>
                <td style={{ padding: '6px 12px', 'text-align': 'right' }}>
                  {formatBytes(stats()!.typed_arrays)}
                </td>
                <td style={{ padding: '6px 0', color: '#888' }}>
                  {pct(stats()!.typed_arrays, stats()!.total)}{' '}
                  <span style={{ 'font-size': '12px' }}>
                    (subset of Native)
                  </span>
                </td>
              </tr>
              <tr>
                <td style={{ padding: '3px 0', color: '#888' }}>Unreachable</td>
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
        </>
      )}
    </div>
  );
}
