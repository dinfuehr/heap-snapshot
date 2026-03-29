import { useEffect, useState, useCallback } from 'react';
import type {
  NodeInfo,
  DominatedChildren,
  ReachableSizeInfo,
} from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import type { NavigateOptions } from '../components/ObjectLink.tsx';
import {
  TreeTableShell,
  TreeTableRow,
  TreeTableMore,
  type RowSelection,
} from '../components/TreeTable.tsx';

interface Props {
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: React.MouseEvent, nodeId: number) => void;
  focusNodeId: number | null;
  reachableSizes: Map<number, ReachableSizeInfo>;
  selection: RowSelection | null;
  onSelect: (sel: RowSelection) => void;
}

function DomTreeNode({
  node,
  call,
  onNavigate,
  onContextMenu,
  reachableSizes,
  selection,
  onSelect,
  depth,
}: {
  node: NodeInfo;
  call: SnapshotCall;
  onNavigate: Props['onNavigate'];
  onContextMenu: Props['onContextMenu'];
  reachableSizes: Map<number, ReachableSizeInfo>;
  selection: RowSelection | null;
  onSelect: (sel: RowSelection) => void;
  depth: number;
}) {
  const [expanded, setExpanded] = useState(false);
  const [children, setChildren] = useState<NodeInfo[] | null>(null);
  const [total, setTotal] = useState(0);

  const toggle = useCallback(async () => {
    if (expanded) {
      setExpanded(false);
      return;
    }
    setExpanded(true);
    if (!children) {
      const result = await call<DominatedChildren>({
        type: 'getDominatedChildren',
        nodeId: node.id,
        offset: 0,
        limit: 100,
      });
      setChildren(result.children);
      setTotal(result.total);
    }
  }, [expanded, children, call, node.id]);

  const label = (
    <>
      {node.name} <span style={{ color: '#888' }}>({node.node_type})</span>
    </>
  );

  return (
    <TreeTableRow
      depth={depth}
      expanded={expanded}
      onToggle={toggle}
      label={label}
      linkId={node.id}
      onNavigate={onNavigate}
      onContextMenu={onContextMenu}
      onSelect={onSelect}
      selection={selection}
      distance={node.distance}
      detachedness={node.detachedness}
      selfSize={node.self_size}
      retainedSize={node.retained_size}
      reachableInfo={reachableSizes.get(node.id)}
    >
      {expanded && children && (
        <>
          {children.map((child, i) => (
            <DomTreeNode
              key={`${child.id}-${i}`}
              node={child}
              call={call}
              onNavigate={onNavigate}
              onContextMenu={onContextMenu}
              reachableSizes={reachableSizes}
              selection={selection}
              onSelect={onSelect}
              depth={depth + 1}
            />
          ))}
          <TreeTableMore
            depth={depth + 1}
            shown={children.length}
            total={total}
            label="dominated children"
          />
        </>
      )}
    </TreeTableRow>
  );
}

export function DominatorsView({
  call,
  onNavigate,
  onContextMenu,
  reachableSizes,
  selection,
  onSelect,
}: Props) {
  const [root, setRoot] = useState<NodeInfo | null>(null);

  useEffect(() => {
    if (!root) {
      call<NodeInfo>({ type: 'getDominatorTreeRoot' }).then(setRoot);
    }
  }, [call, root]);

  if (!root) return <p>Loading...</p>;

  return (
    <TreeTableShell>
      <DomTreeNode
        node={root}
        call={call}
        onNavigate={onNavigate}
        onContextMenu={onContextMenu}
        reachableSizes={reachableSizes}
        selection={selection}
        onSelect={onSelect}
        depth={0}
      />
    </TreeTableShell>
  );
}
