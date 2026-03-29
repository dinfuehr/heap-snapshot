import { useEffect, useState } from 'react';
import type { Statistics } from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import { formatBytes } from '../components/format.ts';

interface Props {
  call: SnapshotCall;
}

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

export function StatisticsView({ call }: Props) {
  const [stats, setStats] = useState<Statistics | null>(null);

  useEffect(() => {
    if (!stats) {
      call<Statistics>({ type: 'getStatistics' }).then(setStats);
    }
  }, [call, stats]);

  if (!stats) return <p>Loading...</p>;

  const total = stats.total;

  return (
    <div style={{ maxWidth: 600, fontVariantNumeric: 'tabular-nums' }}>
      <table
        style={{ borderCollapse: 'collapse', width: '100%', fontSize: 14 }}
      >
        <tbody>
          <tr style={{ borderBottom: '1px solid #ddd' }}>
            <td style={{ padding: '6px 0', fontWeight: 'bold' }}>Total</td>
            <td style={{ padding: '6px 12px', textAlign: 'right' }}>
              {formatBytes(total)}
            </td>
            <td />
          </tr>

          {CATEGORIES.map(({ key, label, color }) => (
            <tr key={key}>
              <td style={{ padding: '3px 0', color, fontWeight: 600 }}>
                {label}
              </td>
              <td style={{ padding: '3px 12px', textAlign: 'right' }}>
                {formatBytes(stats[key] as number)}
              </td>
              <td style={{ padding: '3px 0', color: '#888' }}>
                {pct(stats[key] as number, total)}
              </td>
            </tr>
          ))}

          <tr style={{ borderTop: '1px solid #ddd' }}>
            <td style={{ padding: '6px 0', color: '#888' }}>Typed Arrays</td>
            <td style={{ padding: '6px 12px', textAlign: 'right' }}>
              {formatBytes(stats.typed_arrays)}
            </td>
            <td style={{ padding: '6px 0', color: '#888' }}>
              {pct(stats.typed_arrays, total)}{' '}
              <span style={{ fontSize: 12 }}>(subset of Native)</span>
            </td>
          </tr>

          <tr>
            <td style={{ padding: '3px 0', color: '#888' }}>Unreachable</td>
            <td style={{ padding: '3px 12px', textAlign: 'right' }}>
              {formatBytes(stats.unreachable_size)}
            </td>
            <td style={{ padding: '3px 0', color: '#888', fontSize: 12 }}>
              ({stats.unreachable_count.toLocaleString()} objects)
            </td>
          </tr>
        </tbody>
      </table>

      {/* Stacked bar chart */}
      <div
        style={{
          display: 'flex',
          height: 24,
          borderRadius: 4,
          overflow: 'hidden',
          marginTop: 16,
        }}
      >
        {CATEGORIES.map(({ key, label, color }) => {
          const w = total > 0 ? ((stats[key] as number) / total) * 100 : 0;
          if (w < 0.3) return null;
          return (
            <div
              key={key}
              title={`${label}: ${formatBytes(stats[key] as number)} (${pct(stats[key] as number, total)})`}
              style={{
                width: `${w}%`,
                backgroundColor: color,
                minWidth: w > 0 ? 2 : 0,
              }}
            />
          );
        })}
      </div>

      {/* Legend */}
      <div
        style={{
          display: 'flex',
          gap: 16,
          marginTop: 8,
          fontSize: 12,
          flexWrap: 'wrap',
        }}
      >
        {CATEGORIES.map(({ key, label, color }) => (
          <div
            key={key}
            style={{ display: 'flex', alignItems: 'center', gap: 4 }}
          >
            <div
              style={{
                width: 10,
                height: 10,
                backgroundColor: color,
                borderRadius: 2,
              }}
            />
            <span>{label}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
