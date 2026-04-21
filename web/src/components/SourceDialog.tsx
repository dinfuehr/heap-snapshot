import { onCleanup, onMount, type JSX } from 'solid-js';

export function SourceDialog(props: {
  title: string;
  source: string;
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
        'max-width': '80vw',
        'font-size': '13px',
      }}
    >
      <div
        style={{
          'font-weight': '600',
          'margin-bottom': '12px',
          'font-size': '14px',
          'word-break': 'break-all',
        }}
      >
        {props.title}
      </div>
      <pre
        style={{
          margin: '0',
          padding: '8px',
          background: '#f6f8fa',
          border: '1px solid #e1e4e8',
          'border-radius': '4px',
          'font-family':
            'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
          'font-size': '12px',
          'white-space': 'pre',
          overflow: 'auto',
          'max-height': '60vh',
        }}
      >
        {props.source}
      </pre>
    </div>
  );
}
