import type { JSX } from 'solid-js';
import { For } from 'solid-js';

export function TabNav<T extends string>(props: {
  tabs: readonly T[];
  active: T;
  onChange: (tab: T) => void;
  disabled?: ReadonlySet<T>;
}): JSX.Element {
  return (
    <div
      style={{ display: 'flex', gap: '0px', 'border-bottom': '1px solid #ccc' }}
    >
      <For each={props.tabs}>
        {(tab, i) => {
          const isDisabled = () => props.disabled?.has(tab) ?? false;
          return (
            <button
              onClick={() => !isDisabled() && props.onChange(tab)}
              disabled={isDisabled()}
              style={{
                padding: '8px 16px',
                border: 'none',
                'border-bottom':
                  tab === props.active
                    ? '2px solid #333'
                    : '2px solid transparent',
                background: 'none',
                cursor: isDisabled() ? 'default' : 'pointer',
                'font-weight': tab === props.active ? 'bold' : 'normal',
                'font-size': '14px',
                opacity: isDisabled() ? 0.4 : 1,
              }}
            >
              <span style={{ color: '#888', 'margin-right': '2px' }}>
                {i() + 1}
              </span>{' '}
              {tab}
            </button>
          );
        }}
      </For>
    </div>
  );
}
