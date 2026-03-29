import { useEffect, useState, useCallback } from 'react';
import type {
  NodeInfo,
  Retainers,
  Retainer,
  RetainingPaths,
  RetainingPath,
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
import { formatBytes } from '../components/format.ts';

interface Props {
  call: SnapshotCall;
  nodeId: number | null;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: React.MouseEvent, nodeId: number) => void;
  reachableSizes: Map<number, ReachableSizeInfo>;
  selection: RowSelection | null;
  onSelect: (sel: RowSelection) => void;
}

function PathNode({
  path,
  depth,
  onNavigate,
  onContextMenu,
  reachableSizes,
  selection,
  onSelect,
}: {
  path: RetainingPath;
  depth: number;
  onNavigate: Props['onNavigate'];
  onContextMenu: Props['onContextMenu'];
  reachableSizes: Map<number, ReachableSizeInfo>;
  selection: RowSelection | null;
  onSelect: (sel: RowSelection) => void;
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
      onSelect={onSelect}
      selection={selection}
      distance={path.node.distance}
      detachedness={path.node.detachedness}
      selfSize={path.node.self_size}
      retainedSize={path.node.retained_size}
      reachableInfo={reachableSizes.get(path.node.id)}
    >
      {path.children.map((child, i) => (
        <PathNode
          key={i}
          path={child}
          depth={depth + 1}
          onNavigate={onNavigate}
          onContextMenu={onContextMenu}
          reachableSizes={reachableSizes}
          selection={selection}
          onSelect={onSelect}
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
  reachableSizes,
  selection,
  onSelect,
  depth,
}: {
  retainer: Retainer;
  call: SnapshotCall;
  onNavigate: Props['onNavigate'];
  onContextMenu: Props['onContextMenu'];
  reachableSizes: Map<number, ReachableSizeInfo>;
  selection: RowSelection | null;
  onSelect: (sel: RowSelection) => void;
  depth: number;
}) {
  const [expanded, setExpanded] = useState(false);
  const [children, setChildren] = useState<Retainer[] | null>(null);
  const [total, setTotal] = useState(0);

  const toggle = useCallback(async () => {
    if (expanded) {
      setExpanded(false);
      return;
    }
    setExpanded(true);
    if (!children) {
      const result = await call<Retainers>({
        type: 'getRetainers',
        nodeId: retainer.source.id,
        offset: 0,
        limit: 50,
      });
      setChildren(result.retainers);
      setTotal(result.total);
    }
  }, [expanded, children, call, retainer.source.id]);

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
      onSelect={onSelect}
      selection={selection}
      distance={retainer.source.distance}
      detachedness={retainer.source.detachedness}
      selfSize={retainer.source.self_size}
      retainedSize={retainer.source.retained_size}
      reachableInfo={reachableSizes.get(retainer.source.id)}
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
            label="retainers"
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
  reachableSizes,
  selection,
  onSelect,
}: Props) {
  const [nodeInfo, setNodeInfo] = useState<NodeInfo | null>(null);
  const [retainers, setRetainers] = useState<Retainers | null>(null);
  const [paths, setPaths] = useState<RetainingPaths | null>(null);
  const [inputId, setInputId] = useState(nodeId ? `@${nodeId}` : '');
  const [activeId, setActiveId] = useState<number | null>(nodeId);

  useEffect(() => {
    if (nodeId !== null) {
      setInputId(`@${nodeId}`);
      setActiveId(nodeId);
    }
  }, [nodeId]);

  useEffect(() => {
    if (activeId === null) return;
    setPaths(null);
    setRetainers(null);
    setNodeInfo(null);
    call<NodeInfo>({ type: 'getNodeInfo', nodeId: activeId }).then(setNodeInfo);
    call<Retainers>({
      type: 'getRetainers',
      nodeId: activeId,
      offset: 0,
      limit: 50,
    }).then(setRetainers);
  }, [activeId, call]);

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

      {retainers && (
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
                reachableSizes={reachableSizes}
                selection={selection}
                onSelect={onSelect}
                depth={0}
              />
            ))}
            <TreeTableMore
              depth={0}
              shown={retainers.retainers.length}
              total={retainers.total}
              label="retainers"
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
                    reachableSizes={reachableSizes}
                    selection={selection}
                    onSelect={onSelect}
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
