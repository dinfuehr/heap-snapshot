import { useEffect, useState, useCallback } from 'react';
import type { Containment, Children, NodeInfo } from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import type { NavigateOptions } from '../components/ObjectLink.tsx';
import { TreeTableShell, TreeTableRow } from '../components/TreeTable.tsx';
import { TreeTablePager } from '../components/TreeTablePager.tsx';

interface Props {
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: React.MouseEvent, nodeId: number) => void;
}

const PAGE_SIZE = 100;

function TreeNode({
  edgeLabel,
  node,
  call,
  onNavigate,
  onContextMenu,
  depth,
  initialExpanded = false,
}: {
  edgeLabel?: string;
  node: NodeInfo;
  call: SnapshotCall;
  onNavigate: Props['onNavigate'];
  onContextMenu: Props['onContextMenu'];
  depth: number;
  initialExpanded?: boolean;
}) {
  const [expanded, setExpanded] = useState(initialExpanded);
  const [children, setChildren] = useState<
    { edgeLabel: string; node: NodeInfo }[] | null
  >(null);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [limit, setLimit] = useState(PAGE_SIZE);
  const [filter, setFilter] = useState('');

  const loadChildren = useCallback(
    async (o: number, l: number, f: string) => {
      const result = await call<Children>({
        type: 'getChildren',
        nodeId: node.id,
        offset: o,
        limit: l,
        filter: f,
      });
      setChildren(
        result.edges.map((e) => ({
          edgeLabel: `[${e.edge_name}] `,
          node: e.target,
        })),
      );
      setTotal(result.total);
      setOffset(o);
      setLimit(l);
      setFilter(f);
    },
    [call, node.id],
  );

  useEffect(() => {
    if (initialExpanded && !children) {
      loadChildren(0, PAGE_SIZE, '');
    }
  }, [initialExpanded, children, loadChildren]);

  const toggle = useCallback(async () => {
    if (expanded) {
      setExpanded(false);
      return;
    }
    setExpanded(true);
    if (!children) {
      await loadChildren(0, PAGE_SIZE, '');
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
      detachedness={node.detachedness}
      distance={node.distance}
      selfSize={node.self_size}
      retainedSize={node.retained_size}
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
              depth={depth + 1}
            />
          ))}
          <TreeTablePager
            depth={depth + 1}
            shown={children.length}
            total={total}
            offset={offset}
            filter={filter}
            onPageChange={(o, l) => loadChildren(o, l, filter)}
            onFilterChange={(f) => loadChildren(0, limit, f)}
            onShowAll={() => loadChildren(0, 999999, filter)}
          />
        </>
      )}
    </TreeTableRow>
  );
}

export function ContainmentView({ call, onNavigate, onContextMenu }: Props) {
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
          depth={0}
          initialExpanded={edge.target.name === '(GC roots)'}
        />
      ))}
    </TreeTableShell>
  );
}
