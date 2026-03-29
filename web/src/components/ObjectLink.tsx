import type { JSX } from 'solid-js';

export interface NavigateOptions {
  nodeId: number;
  target: 'Retainers' | 'Dominators' | 'Summary';
}

export function ObjectLink(props: {
  nodeId: number;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
}): JSX.Element {
  return (
    <a
      href="#"
      onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        props.onNavigate({ nodeId: props.nodeId, target: 'Retainers' });
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        e.stopPropagation();
        props.onContextMenu(e, props.nodeId);
      }}
      style={{ color: '#06c' }}
    >
      @{props.nodeId}
    </a>
  );
}
