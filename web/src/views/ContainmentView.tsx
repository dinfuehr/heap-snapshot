import { useEffect, useState, useCallback } from 'react';
import type {
  Containment,
  Children,
  NodeInfo,
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
  reachableSizes: Map<number, ReachableSizeInfo>;
  selection: RowSelection | null;
  onSelect: (sel: RowSelection) => void;
}

function TreeNode({
  edgeLabel,
  node,
  call,
  onNavigate,
  onContextMenu,
  reachableSizes,
  selection,
  onSelect,
  depth,
  initialExpanded = false,
}: {
  edgeLabel?: string;
  node: NodeInfo;
  call: SnapshotCall;
  onNavigate: Props['onNavigate'];
  onContextMenu: Props['onContextMenu'];
  reachableSizes: Map<number, ReachableSizeInfo>;
  selection: RowSelection | null;
  onSelect: (sel: RowSelection) => void;
  depth: number;
  initialExpanded?: boolean;
}) {
  const [expanded, setExpanded] = useState(initialExpanded);
  const [children, setChildren] = useState<
    { edgeLabel: string; node: NodeInfo }[] | null
  >(null);
  const [total, setTotal] = useState(0);

  const loadChildren = useCallback(async () => {
    const result = await call<Children>({
      type: 'getChildren',
      nodeId: node.id,
      offset: 0,
      limit: 100,
    });
    setChildren(
      result.edges.map((e) => ({
        edgeLabel: `[${e.edge_name}] `,
        node: e.target,
      })),
    );
    setTotal(result.total);
  }, [call, node.id]);

  useEffect(() => {
    if (initialExpanded && !children) {
      loadChildren();
    }
  }, [initialExpanded, children, loadChildren]);

  const toggle = useCallback(async () => {
    if (expanded) {
      setExpanded(false);
      return;
    }
    setExpanded(true);
    if (!children) {
      await loadChildren();
    }
  }, [expanded, children, loadChildren]);

  const hasChildren = node.edge_count > 0;
  const label = (
    <>
      {edgeLabel && <span style={{ color: '#888' }}>{edgeLabel}</span>}
      {node.name} <span style={{ color: '#888' }}>({node.node_type})</span>
    </>
  );

  return (
    <TreeTableRow
      depth={depth}
      expanded={expanded}
      onToggle={hasChildren ? toggle : undefined}
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
            <TreeNode
              key={`${child.node.id}-${i}`}
              edgeLabel={child.edgeLabel}
              node={child.node}
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
          />
        </>
      )}
    </TreeTableRow>
  );
}

export function ContainmentView({
  call,
  onNavigate,
  onContextMenu,
  reachableSizes,
  selection,
  onSelect,
}: Props) {
  const [containment, setContainment] = useState<Containment | null>(null);

  useEffect(() => {
    if (!containment) {
      call<Containment>({ type: 'getContainment' }).then(setContainment);
    }
  }, [call, containment]);

  if (!containment) return <p>Loading...</p>;

  return (
    <TreeTableShell>
      {containment.system_roots.map((edge, i) => (
        <TreeNode
          key={`sr-${i}`}
          edgeLabel={`[${edge.edge_name}] `}
          node={edge.target}
          call={call}
          onNavigate={onNavigate}
          onContextMenu={onContextMenu}
          reachableSizes={reachableSizes}
          selection={selection}
          onSelect={onSelect}
          depth={0}
          initialExpanded={edge.target.name === '(GC roots)'}
        />
      ))}
    </TreeTableShell>
  );
}
