import { onCleanup, onMount, type JSX } from 'solid-js';
import { For } from 'solid-js';

export interface ContextMenuItem {
  label: string;
  action: () => void;
}

export function ContextMenu(props: {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}): JSX.Element {
  let ref: HTMLDivElement | undefined;

  onMount(() => {
    const handler = (e: MouseEvent) => {
      if (ref && !ref.contains(e.target as Node)) {
        props.onClose();
      }
    };
    document.addEventListener('mousedown', handler);
    onCleanup(() => document.removeEventListener('mousedown', handler));
  });

  return (
    <div
      ref={ref}
      style={{
        position: 'fixed',
        left: `${props.x}px`,
        top: `${props.y}px`,
        background: '#fff',
        border: '1px solid #ccc',
        'border-radius': '4px',
        'box-shadow': '0 2px 8px rgba(0,0,0,0.15)',
        'z-index': 1000,
        padding: '4px 0',
        'min-width': '160px',
        'font-size': '13px',
      }}
    >
      <For each={props.items}>
        {(item) => (
          <div
            onClick={() => {
              item.action();
              props.onClose();
            }}
            style={{ padding: '6px 12px', cursor: 'pointer' }}
            onMouseEnter={(e) => (e.currentTarget.style.background = '#f0f0f0')}
            onMouseLeave={(e) =>
              (e.currentTarget.style.background = 'transparent')
            }
          >
            {item.label}
          </div>
        )}
      </For>
    </div>
  );
}
