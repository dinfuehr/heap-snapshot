import { useEffect, useState, useCallback } from 'react';
import type {
  NodeInfo,
  Retainers,
  Retainer,
  RetainingPaths,
  RetainingPath,
} from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import type { NavigateOptions } from '../components/ObjectLink.tsx';
import { TreeTableShell, TreeTableRow } from '../components/TreeTable.tsx';
import { TreeTablePager } from '../components/TreeTablePager.tsx';
import { formatBytes } from '../components/format.ts';

interface Props {
  call: SnapshotCall;
  nodeId: number | null;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: React.MouseEvent, nodeId: number) => void;
}

const PAGE_SIZE = 100;

function PathNode({
  path,
  depth,
  onNavigate,
  onContextMenu,
}: {
  path: RetainingPath;
  depth: number;
  onNavigate: Props['onNavigate'];
  onContextMenu: Props['onContextMenu'];
}) {
  const label = (
    <>
      <span style={{ color: '#888' }}>[{path.edge_name}]</span> {path.node.name}{' '}
      <span style={{ color: '#888' }}>({path.node.node_type})</span>
    </>
  );

  return (
    <TreeTableRow
      depth={depth}
      label={label}
      linkId={path.node.id}
      onNavigate={onNavigate}
      onContextMenu={onContextMenu}
      detachedness={path.node.detachedness}
      distance={path.node.distance}
      selfSize={path.node.self_size}
      retainedSize={path.node.retained_size}
    >
      {path.children.map((child, i) => (
        <PathNode
          key={i}
          path={child}
          depth={depth + 1}
          onNavigate={onNavigate}
          onContextMenu={onContextMenu}
        />
      ))}
    </TreeTableRow>
  );
}

function RetainerRow({
  retainer,
  call,
  onNavigate,
  onContextMenu,
  depth,
}: {
  retainer: Retainer;
  call: SnapshotCall;
  onNavigate: Props['onNavigate'];
  onContextMenu: Props['onContextMenu'];
  depth: number;
}) {
  const [expanded, setExpanded] = useState(false);
  const [children, setChildren] = useState<Retainer[] | null>(null);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [limit, setLimit] = useState(PAGE_SIZE);
  const [filter, setFilter] = useState('');

  const loadRetainers = useCallback(
    async (o: number, l: number, f: string) => {
      const result = await call<Retainers>({
        type: 'getRetainers',
        nodeId: retainer.source.id,
        offset: o,
        limit: l,
        filter: f,
      });
      setChildren(result.retainers);
      setTotal(result.total);
      setOffset(o);
      setLimit(l);
      setFilter(f);
    },
    [call, retainer.source.id],
  );

  const toggle = useCallback(async () => {
    if (expanded) {
      setExpanded(false);
      return;
    }
    setExpanded(true);
    if (!children) {
      await loadRetainers(0, PAGE_SIZE, '');
    }
  }, [expanded, children, loadRetainers]);

  const label = (
    <>
      <span style={{ color: '#888' }}>[{retainer.edge_name}]</span>
      {' in '}
      {retainer.source.name}{' '}
      <span style={{ color: '#888' }}>({retainer.source.node_type})</span>
    </>
  );

  return (
    <TreeTableRow
      depth={depth}
      expanded={expanded}
      onToggle={toggle}
      label={label}
      linkId={retainer.source.id}
      onNavigate={onNavigate}
      onContextMenu={onContextMenu}
      detachedness={retainer.source.detachedness}
      distance={retainer.source.distance}
      selfSize={retainer.source.self_size}
      retainedSize={retainer.source.retained_size}
    >
      {expanded && children && (
        <>
          {children.map((r, i) => (
            <RetainerRow
              key={i}
              retainer={r}
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
            onPageChange={(o, l) => loadRetainers(o, l, filter)}
            onFilterChange={(f) => loadRetainers(0, limit, f)}
            onShowAll={() => loadRetainers(0, 999999, filter)}
          />
        </>
      )}
    </TreeTableRow>
  );
}

export function RetainersView({
  call,
  nodeId,
  onNavigate,
  onContextMenu,
}: Props) {
  const [nodeInfo, setNodeInfo] = useState<NodeInfo | null>(null);
  const [retainers, setRetainers] = useState<Retainers | null>(null);
  const [retOffset, setRetOffset] = useState(0);
  const [retLimit, setRetLimit] = useState(PAGE_SIZE);
  const [retFilter, setRetFilter] = useState('');
  const [paths, setPaths] = useState<RetainingPaths | null>(null);
  const [inputId, setInputId] = useState(nodeId ? `@${nodeId}` : '');
  const [activeId, setActiveId] = useState<number | null>(nodeId);

  useEffect(() => {
    if (nodeId !== null) {
      setInputId(`@${nodeId}`);
      setActiveId(nodeId);
    }
  }, [nodeId]);

  const loadRetainers = useCallback(
    async (id: number, o: number, l: number, f: string) => {
      const result = await call<Retainers>({
        type: 'getRetainers',
        nodeId: id,
        offset: o,
        limit: l,
        filter: f,
      });
      setRetainers(result);
      setRetOffset(o);
      setRetLimit(l);
      setRetFilter(f);
    },
    [call],
  );

  useEffect(() => {
    if (activeId === null) return;
    setPaths(null);
    setRetainers(null);
    setNodeInfo(null);
    call<NodeInfo>({ type: 'getNodeInfo', nodeId: activeId }).then(setNodeInfo);
    loadRetainers(activeId, 0, PAGE_SIZE, '');
  }, [activeId, call, loadRetainers]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const raw = inputId.replace(/^@/, '');
    const id = parseInt(raw, 10);
    if (!isNaN(id)) {
      setActiveId(id);
    }
  };

  const loadPaths = async () => {
    if (activeId === null) return;
    const result = await call<RetainingPaths>({
      type: 'getRetainingPaths',
      nodeId: activeId,
      maxDepth: 50,
      maxNodes: 200,
    });
    setPaths(result);
  };

  return (
    <div>
      <form onSubmit={handleSubmit} style={{ marginBottom: 16 }}>
        <input
          value={inputId}
          onChange={(e) => setInputId(e.target.value)}
          placeholder="@12345"
          style={{ padding: '4px 8px', fontSize: 14, marginRight: 8 }}
        />
        <button type="submit" style={{ padding: '4px 12px', fontSize: 14 }}>
          Go
        </button>
      </form>

      {nodeInfo && (
        <div style={{ marginBottom: 16 }}>
          <strong>@{nodeInfo.id}</strong> {nodeInfo.name}{' '}
          <span style={{ color: '#888' }}>
            (type: {nodeInfo.node_type}, self: {formatBytes(nodeInfo.self_size)}
            , retained: {formatBytes(nodeInfo.retained_size)}, distance:{' '}
            {nodeInfo.distance})
          </span>
        </div>
      )}

      {retainers && activeId !== null && (
        <>
          <h3 style={{ fontSize: 14, margin: '0 0 8px' }}>
            Direct Retainers ({retainers.total})
          </h3>
          <TreeTableShell>
            {retainers.retainers.map((r, i) => (
              <RetainerRow
                key={i}
                retainer={r}
                call={call}
                onNavigate={onNavigate}
                onContextMenu={onContextMenu}
                depth={0}
              />
            ))}
            <TreeTablePager
              depth={0}
              shown={retainers.retainers.length}
              total={retainers.total}
              offset={retOffset}
              filter={retFilter}
              onPageChange={(o, l) => loadRetainers(activeId, o, l, retFilter)}
              onFilterChange={(f) => loadRetainers(activeId, 0, retLimit, f)}
              onShowAll={() => loadRetainers(activeId, 0, 999999, retFilter)}
            />
          </TreeTableShell>
        </>
      )}

      {activeId !== null && (
        <div style={{ marginTop: 16 }}>
          {paths ? (
            <>
              <h3 style={{ fontSize: 14, margin: '0 0 8px' }}>
                Retaining Paths to GC Roots
                {paths.truncated && ' (truncated)'}
                {!paths.reached_gc_roots && ' (GC roots not reached)'}
              </h3>
              <TreeTableShell>
                {paths.paths.map((p, i) => (
                  <PathNode
                    key={i}
                    path={p}
                    depth={0}
                    onNavigate={onNavigate}
                    onContextMenu={onContextMenu}
                  />
                ))}
              </TreeTableShell>
            </>
          ) : (
            <button
              onClick={loadPaths}
              style={{ padding: '4px 12px', fontSize: 14 }}
            >
              Find retaining paths to GC roots
            </button>
          )}
        </div>
      )}
    </div>
  );
}
