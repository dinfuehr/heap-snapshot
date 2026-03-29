import { useEffect, useRef } from 'react';

export interface ContextMenuItem {
  label: string;
  action: () => void;
}

interface Props {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}

export function ContextMenu({ x, y, items, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [onClose]);

  return (
    <div
      ref={ref}
      style={{
        position: 'fixed',
        left: x,
        top: y,
        background: '#fff',
        border: '1px solid #ccc',
        borderRadius: 4,
        boxShadow: '0 2px 8px rgba(0,0,0,0.15)',
        zIndex: 1000,
        padding: '4px 0',
        minWidth: 160,
        fontSize: 13,
      }}
    >
      {items.map((item, i) => (
        <div
          key={i}
          onClick={() => {
            item.action();
            onClose();
          }}
          style={{
            padding: '6px 12px',
            cursor: 'pointer',
          }}
          onMouseEnter={(e) => (e.currentTarget.style.background = '#f0f0f0')}
          onMouseLeave={(e) =>
            (e.currentTarget.style.background = 'transparent')
          }
        >
          {item.label}
        </div>
      ))}
    </div>
  );
}
