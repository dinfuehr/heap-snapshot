import { useState, useCallback } from 'react';
import type { NodeInfo, Children } from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import type { NavigateOptions } from '../components/ObjectLink.tsx';
import { TreeTableShell, TreeTableRow } from '../components/TreeTable.tsx';
import { TreeTablePager } from '../components/TreeTablePager.tsx';

interface Props {
  call: SnapshotCall;
  history: NodeInfo[];
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: React.MouseEvent, nodeId: number) => void;
}

const PAGE_SIZE = 100;

function HistoryEntry({
  node,
  call,
  onNavigate,
  onContextMenu,
}: {
  node: NodeInfo;
  call: SnapshotCall;
  onNavigate: Props['onNavigate'];
  onContextMenu: Props['onContextMenu'];
}) {
  const [expanded, setExpanded] = useState(false);
  const [children, setChildren] = useState<
    { edgeType: string; edgeName: string; node: NodeInfo }[] | null
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
          edgeType: e.edge_type,
          edgeName: e.edge_name,
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
      {node.name} <span style={{ color: '#888' }}>({node.node_type})</span>
    </>
  );

  return (
    <TreeTableRow
      depth={0}
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
          {children.map((child, i) => {
            const childLabel = (
              <>
                <span style={{ color: '#888' }}>[{child.edgeName}]</span>{' '}
                {child.node.name}{' '}
                <span style={{ color: '#888' }}>({child.node.node_type})</span>
              </>
            );
            return (
              <TreeTableRow
                key={`${child.node.id}-${i}`}
                depth={1}
                label={childLabel}
                linkId={child.node.id}
                onNavigate={onNavigate}
                onContextMenu={onContextMenu}
                detachedness={child.node.detachedness}
                distance={child.node.distance}
                selfSize={child.node.self_size}
                retainedSize={child.node.retained_size}
              />
            );
          })}
          <TreeTablePager
            depth={1}
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

export function HistoryView({
  call,
  history,
  onNavigate,
  onContextMenu,
}: Props) {
  if (history.length === 0) {
    return (
      <p style={{ color: '#888', fontSize: 13 }}>
        No history yet. Navigate to objects via the Retainers view or
        right-click context menu.
      </p>
    );
  }

  return (
    <TreeTableShell>
      {[...history].reverse().map((node, i) => (
        <HistoryEntry
          key={`${node.id}-${i}`}
          node={node}
          call={call}
          onNavigate={onNavigate}
          onContextMenu={onContextMenu}
        />
      ))}
    </TreeTableShell>
  );
}
