import { useEffect, useState, useCallback } from 'react';
import type { NodeInfo, DominatedChildren } from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import type { NavigateOptions } from '../components/ObjectLink.tsx';
import { TreeTableShell, TreeTableRow } from '../components/TreeTable.tsx';
import { TreeTablePager } from '../components/TreeTablePager.tsx';

interface Props {
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: React.MouseEvent, nodeId: number) => void;
  focusNodeId: number | null;
}

const PAGE_SIZE = 100;

function DomTreeNode({
  node,
  call,
  onNavigate,
  onContextMenu,
  depth,
}: {
  node: NodeInfo;
  call: SnapshotCall;
  onNavigate: Props['onNavigate'];
  onContextMenu: Props['onContextMenu'];
  depth: number;
}) {
  const [expanded, setExpanded] = useState(false);
  const [children, setChildren] = useState<NodeInfo[] | null>(null);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);

  const loadChildren = useCallback(
    async (o: number, l: number) => {
      const result = await call<DominatedChildren>({
        type: 'getDominatedChildren',
        nodeId: node.id,
        offset: o,
        limit: l,
      });
      setChildren(result.children);
      setTotal(result.total);
      setOffset(o);
    },
    [call, node.id],
  );

  const toggle = useCallback(async () => {
    if (expanded) {
      setExpanded(false);
      return;
    }
    setExpanded(true);
    if (!children) {
      await loadChildren(0, PAGE_SIZE);
    }
  }, [expanded, children, loadChildren]);

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
      detachedness={node.detachedness}
      distance={node.distance}
      selfSize={node.self_size}
      retainedSize={node.retained_size}
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
              depth={depth + 1}
            />
          ))}
          <TreeTablePager
            depth={depth + 1}
            shown={children.length}
            total={total}
            offset={offset}
            filter=""
            onPageChange={(o, l) => loadChildren(o, l)}
            onFilterChange={() => {}}
            onShowAll={() => loadChildren(0, 999999)}
          />
        </>
      )}
    </TreeTableRow>
  );
}

export function DominatorsView({ call, onNavigate, onContextMenu }: Props) {
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
        depth={0}
      />
    </TreeTableShell>
  );
}
