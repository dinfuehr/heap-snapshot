import { useEffect, useState, useCallback } from 'react';
import type { NativeContext, Children, NodeInfo } from '../types.ts';
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

function ContextNode({
  ctx,
  call,
  onNavigate,
  onContextMenu,
}: {
  ctx: NativeContext;
  call: SnapshotCall;
  onNavigate: Props['onNavigate'];
  onContextMenu: Props['onContextMenu'];
}) {
  const [expanded, setExpanded] = useState(false);
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
        nodeId: ctx.id,
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
    [call, ctx.id],
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

  const label = (
    <>
      {ctx.label}
      {ctx.vars && (
        <span style={{ color: '#888', fontSize: 11, marginLeft: 8 }}>
          Vars: {ctx.vars}
        </span>
      )}
    </>
  );

  return (
    <TreeTableRow
      depth={0}
      expanded={expanded}
      onToggle={toggle}
      label={label}
      linkId={ctx.id}
      onNavigate={onNavigate}
      onContextMenu={onContextMenu}
      detachedness={
        ctx.detachedness === 'detached'
          ? 2
          : ctx.detachedness === 'attached'
            ? 1
            : 0
      }
      selfSize={ctx.self_size}
      retainedSize={ctx.retained_size}
    >
      {expanded && children && (
        <>
          {children.map((child, i) => (
            <TreeTableRow
              key={`${child.node.id}-${i}`}
              depth={1}
              label={
                <>
                  <span style={{ color: '#888' }}>{child.edgeLabel}</span>
                  {child.node.name}{' '}
                  <span style={{ color: '#888' }}>
                    ({child.node.node_type})
                  </span>
                </>
              }
              linkId={child.node.id}
              onNavigate={onNavigate}
              onContextMenu={onContextMenu}
              detachedness={child.node.detachedness}
              distance={child.node.distance}
              selfSize={child.node.self_size}
              retainedSize={child.node.retained_size}
            />
          ))}
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

export function ContextsView({ call, onNavigate, onContextMenu }: Props) {
  const [contexts, setContexts] = useState<NativeContext[] | null>(null);

  useEffect(() => {
    if (!contexts) {
      call<NativeContext[]>({ type: 'getNativeContexts' }).then(setContexts);
    }
  }, [call, contexts]);

  if (!contexts) return <p>Loading...</p>;

  if (contexts.length === 0) return <p>No native contexts found.</p>;

  return (
    <div>
      <p style={{ fontSize: 13, color: '#888', margin: '0 0 8px' }}>
        {contexts.length} native context{contexts.length !== 1 ? 's' : ''}{' '}
        (JavaScript realms)
      </p>
      <TreeTableShell>
        {contexts.map((ctx) => (
          <ContextNode
            key={ctx.id}
            ctx={ctx}
            call={call}
            onNavigate={onNavigate}
            onContextMenu={onContextMenu}
          />
        ))}
      </TreeTableShell>
    </div>
  );
}
