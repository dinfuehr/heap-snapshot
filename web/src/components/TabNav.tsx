interface Props<T extends string> {
  tabs: readonly T[];
  active: T;
  onChange: (tab: T) => void;
}

export function TabNav<T extends string>({ tabs, active, onChange }: Props<T>) {
  return (
    <div style={{ display: 'flex', gap: 0, borderBottom: '1px solid #ccc' }}>
      {tabs.map((tab, i) => (
        <button
          key={tab}
          onClick={() => onChange(tab)}
          style={{
            padding: '8px 16px',
            border: 'none',
            borderBottom:
              tab === active ? '2px solid #333' : '2px solid transparent',
            background: 'none',
            cursor: 'pointer',
            fontWeight: tab === active ? 'bold' : 'normal',
            fontSize: 14,
          }}
        >
          <span style={{ color: '#888', marginRight: 2 }}>{i + 1}</span> {tab}
        </button>
      ))}
    </div>
  );
}
