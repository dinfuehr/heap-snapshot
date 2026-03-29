import { useCallback } from 'react';

export interface NavigateOptions {
  nodeId: number;
  target: 'Retainers' | 'Dominators' | 'Summary';
}

interface Props {
  nodeId: number;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: React.MouseEvent, nodeId: number) => void;
}

export function ObjectLink({ nodeId, onNavigate, onContextMenu }: Props) {
  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      onNavigate({ nodeId, target: 'Retainers' });
    },
    [nodeId, onNavigate],
  );

  const handleContext = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      onContextMenu(e, nodeId);
    },
    [nodeId, onContextMenu],
  );

  return (
    <a
      href="#"
      onClick={handleClick}
      onContextMenu={handleContext}
      style={{ color: '#06c' }}
    >
      @{nodeId}
    </a>
  );
}
