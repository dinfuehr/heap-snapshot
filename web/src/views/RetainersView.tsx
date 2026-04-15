import {
  createSignal,
  createEffect,
  Show,
  For,
  type JSX,
  type Accessor,
} from 'solid-js';
import type {
  NodeInfo,
  RetainingPaths,
  RetainingPath,
  Retainers,
  Retainer,
  ReachableSizeInfo,
} from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import type { NavigateOptions } from '../components/ObjectLink.tsx';
import {
  TreeTableShell,
  TreeTableRow,
  TreeTableLoading,
  type RowSelection,
} from '../components/TreeTable.tsx';
import { TreeTablePager } from '../components/TreeTablePager.tsx';
import { formatBytes } from '../components/format.ts';

const PAGE_SIZE = 100;

const btnStyle = {
  padding: '1px 6px',
  'font-size': '11px',
  cursor: 'pointer',
  border: '1px solid #ccc',
  'border-radius': '3px',
  background: '#fafafa',
};

function edgeLabel(edge_type: string, edge_name: string): string {
  return edge_type === 'element' || edge_type === 'hidden'
    ? `[${edge_name}] in `
    : `${edge_name} in `;
}

/** Summary row: "N of M retainers [View]" shown under pre-computed children. */
function RetainerSummary(props: {
  depth: number;
  shown: number;
  total: number | null;
  onView: () => void;
}): JSX.Element {
  return (
    <Show when={props.total !== null && props.total! > props.shown}>
      <tr>
        <td
          colSpan={6}
          style={{
            padding: '4px 8px',
            'padding-left': `${8 + props.depth * 16}px`,
            'font-size': '11px',
            color: '#888',
          }}
        >
          <span
            style={{
              display: 'inline-flex',
              'align-items': 'center',
              gap: '6px',
            }}
          >
            {props.shown} selected of {props.total} retainers
            <button
              style={btnStyle}
              onClick={(e) => {
                e.stopPropagation();
                props.onView();
              }}
            >
              View all
            </button>
          </span>
        </td>
      </tr>
    </Show>
  );
}

function RetainerNode(props: {
  edgeLabel: string;
  node: NodeInfo;
  depth: number;
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
  expandGcTarget: Accessor<number | null>;
  selection: () => RowSelection | null;
  onSelect: (sel: RowSelection) => void;
  reachableSizes: Map<number, ReachableSizeInfo>;
  reachablePending: Set<number>;
  initialExpanded?: boolean;
  precomputedChildren?: RetainingPath[];
}): JSX.Element {
  const [expanded, setExpanded] = createSignal(props.initialExpanded ?? false);
  const [loading, setLoading] = createSignal(false);
  const [retainers, setRetainers] = createSignal<Retainer[] | null>(null);
  const [total, setTotal] = createSignal<number | null>(null);
  const [offset, setOffset] = createSignal(0);
  const [limit, setLimit] = createSignal(PAGE_SIZE);
  const [filter, setFilter] = createSignal('');
  // Whether we switched from pre-computed view to full pagination
  const [paginationMode, setPaginationMode] = createSignal(false);

  // GC root paths loaded via context menu action
  const [gcPaths, setGcPaths] = createSignal<RetainingPath[] | null>(null);
  const [gcLoading, setGcLoading] = createSignal(false);

  const loadRetainers = async (o: number, l: number, f: string) => {
    const result = await props.call<Retainers>({
      type: 'getRetainers',
      nodeId: props.node.id,
      offset: o,
      limit: l,
    });
    setRetainers(result.retainers);
    setTotal(result.total);
    setOffset(o);
    setLimit(l);
    setFilter(f);
  };

  // Fetch total retainer count for the summary row when we have pre-computed children
  if (props.precomputedChildren && props.precomputedChildren.length > 0) {
    props
      .call<Retainers>({
        type: 'getRetainers',
        nodeId: props.node.id,
        offset: 0,
        limit: 0,
      })
      .then((result) => setTotal(result.total));
  }

  const toggle = async () => {
    if (expanded()) {
      setExpanded(false);
      return;
    }
    if (
      !retainers() &&
      !props.precomputedChildren?.length &&
      !paginationMode()
    ) {
      setLoading(true);
      await loadRetainers(0, PAGE_SIZE, '');
      setLoading(false);
    }
    setExpanded(true);
  };

  const switchToPagination = async () => {
    setLoading(true);
    setPaginationMode(true);
    await loadRetainers(0, PAGE_SIZE, '');
    setLoading(false);
  };

  // Watch for GC root expansion trigger from context menu
  createEffect(() => {
    const target = props.expandGcTarget();
    if (target === props.node.id && gcPaths() === null && !gcLoading()) {
      setGcLoading(true);
      props
        .call<RetainingPaths>({
          type: 'getRetainingPaths',
          nodeId: props.node.id,
          maxDepth: 50,
          maxNodes: 200,
        })
        .then((result) => {
          setGcPaths(result.paths);
          setGcLoading(false);
          setExpanded(true);
        });
    }
  });

  const hasPrecomputed = () =>
    props.precomputedChildren &&
    props.precomputedChildren.length > 0 &&
    !paginationMode();

  return (
    <TreeTableRow
      depth={props.depth}
      expanded={expanded()}
      loading={loading() || gcLoading()}
      onToggle={toggle}
      prefix={<span style={{ color: '#888' }}>{props.edgeLabel}</span>}
      label={<>{props.node.name}</>}
      linkId={props.node.id}
      onNavigate={props.onNavigate}
      onContextMenu={props.onContextMenu}
      selection={props.selection()}
      onSelect={props.onSelect}
      detachedness={props.node.detachedness}
      ctx={props.node.ctx}
      ctxLabel={props.node.ctx_label}
      distance={props.node.distance}
      selfSize={props.node.self_size}
      retainedSize={props.node.retained_size}
      reachableInfo={props.reachableSizes.get(props.node.id)}
      reachableLoading={props.reachablePending.has(props.node.id)}
    >
      <Show when={expanded()}>
        {/* Pre-computed children from initial getRetainingPaths */}
        <Show when={hasPrecomputed()}>
          <For each={props.precomputedChildren!}>
            {(child) => (
              <RetainerNode
                edgeLabel={edgeLabel(child.edge_type, child.edge_name)}
                node={child.node}
                depth={props.depth + 1}
                call={props.call}
                onNavigate={props.onNavigate}
                onContextMenu={props.onContextMenu}
                expandGcTarget={props.expandGcTarget}
                selection={props.selection}
                onSelect={props.onSelect}
                reachableSizes={props.reachableSizes}
                reachablePending={props.reachablePending}
                initialExpanded={child.children.length > 0}
                precomputedChildren={child.children}
              />
            )}
          </For>
          <RetainerSummary
            depth={props.depth + 1}
            shown={props.precomputedChildren!.length}
            total={total()}
            onView={switchToPagination}
          />
        </Show>
        {/* Retainers loaded via pagination (either from manual expand or "View all") */}
        <Show when={retainers()}>
          {(rets) => (
            <>
              <For each={rets()}>
                {(ret) => (
                  <RetainerNode
                    edgeLabel={edgeLabel(ret.edge_type, ret.edge_name)}
                    node={ret.source}
                    depth={props.depth + 1}
                    call={props.call}
                    onNavigate={props.onNavigate}
                    onContextMenu={props.onContextMenu}
                    expandGcTarget={props.expandGcTarget}
                    selection={props.selection}
                    onSelect={props.onSelect}
                    reachableSizes={props.reachableSizes}
                    reachablePending={props.reachablePending}
                  />
                )}
              </For>
              <TreeTablePager
                depth={props.depth + 1}
                shown={rets().length}
                total={total()!}
                offset={offset()}
                filter={filter()}
                onPageChange={(o, l) => loadRetainers(o, l, filter())}
                onFilterChange={(f) => loadRetainers(0, limit(), f)}
                onShowAll={() => loadRetainers(0, 999999, filter())}
              />
            </>
          )}
        </Show>
        {/* GC root paths loaded via context menu */}
        <Show when={gcPaths()}>
          {(paths) => (
            <For each={paths()}>
              {(child) => (
                <RetainerNode
                  edgeLabel={edgeLabel(child.edge_type, child.edge_name)}
                  node={child.node}
                  depth={props.depth + 1}
                  call={props.call}
                  onNavigate={props.onNavigate}
                  onContextMenu={props.onContextMenu}
                  expandGcTarget={props.expandGcTarget}
                  selection={props.selection}
                  onSelect={props.onSelect}
                  reachableSizes={props.reachableSizes}
                  reachablePending={props.reachablePending}
                  initialExpanded={child.children.length > 0}
                  precomputedChildren={child.children}
                />
              )}
            </For>
          )}
        </Show>
        <Show when={loading() || gcLoading()}>
          <TreeTableLoading depth={props.depth + 1} />
        </Show>
      </Show>
    </TreeTableRow>
  );
}

export function RetainersView(props: {
  call: SnapshotCall;
  nodeId: number | null;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
  expandGcTarget: Accessor<number | null>;
  reachableSizes: Map<number, ReachableSizeInfo>;
  reachablePending: Set<number>;
}): JSX.Element {
  const [nodeInfo, setNodeInfo] = createSignal<NodeInfo | null>(null);
  const [paths, setPaths] = createSignal<RetainingPaths | null>(null);
  const [inputId, setInputId] = createSignal(
    props.nodeId ? `@${props.nodeId}` : '',
  );
  const [activeId, setActiveId] = createSignal<number | null>(props.nodeId);
  const [selection, setSelection] = createSignal<RowSelection | null>(null);

  createEffect(() => {
    if (props.nodeId !== null) {
      setInputId(`@${props.nodeId}`);
      setActiveId(props.nodeId);
    }
  });

  createEffect(() => {
    const id = activeId();
    if (id === null) return;
    setPaths(null);
    setNodeInfo(null);
    props.call<NodeInfo>({ type: 'getNodeInfo', nodeId: id }).then(setNodeInfo);
    props
      .call<RetainingPaths>({
        type: 'getRetainingPaths',
        nodeId: id,
        maxDepth: 50,
        maxNodes: 200,
      })
      .then(setPaths);
  });

  const handleSubmit = (e: Event) => {
    e.preventDefault();
    const raw = inputId().replace(/^@/, '');
    const id = parseInt(raw, 10);
    if (!isNaN(id)) setActiveId(id);
  };

  return (
    <div class="tab-panel">
      <form onSubmit={handleSubmit} style={{ 'margin-bottom': '16px' }}>
        <input
          value={inputId()}
          onInput={(e) => setInputId(e.currentTarget.value)}
          placeholder="@12345"
          style={{
            padding: '4px 8px',
            'font-size': '14px',
            'margin-right': '8px',
          }}
        />
        <button
          type="submit"
          style={{ padding: '4px 12px', 'font-size': '14px' }}
        >
          Go
        </button>
      </form>

      <Show when={nodeInfo()}>
        {(info) => (
          <div style={{ 'margin-bottom': '16px' }}>
            <strong>@{info().id}</strong> {info().name}{' '}
            <span style={{ color: '#888' }}>
              (type: {info().node_type}, self: {formatBytes(info().self_size)},
              retained: {formatBytes(info().retained_size)}, distance:{' '}
              {info().distance})
            </span>
          </div>
        )}
      </Show>

      <Show when={paths()}>
        {(p) => (
          <>
            <h3
              data-testid="retaining-paths-header"
              style={{ 'font-size': '14px', margin: '0 0 8px' }}
            >
              Retaining Paths to GC Roots
              {p().truncated && ' (truncated)'}
              {!p().reached_gc_roots && ' (GC roots not reached)'}
            </h3>
            <TreeTableShell>
              <For each={p().paths}>
                {(path) => (
                  <RetainerNode
                    edgeLabel={edgeLabel(path.edge_type, path.edge_name)}
                    node={path.node}
                    depth={0}
                    call={props.call}
                    onNavigate={props.onNavigate}
                    onContextMenu={props.onContextMenu}
                    expandGcTarget={props.expandGcTarget}
                    selection={selection}
                    onSelect={setSelection}
                    reachableSizes={props.reachableSizes}
                    reachablePending={props.reachablePending}
                    initialExpanded={path.children.length > 0}
                    precomputedChildren={path.children}
                  />
                )}
              </For>
            </TreeTableShell>
          </>
        )}
      </Show>

      <Show when={activeId() !== null && !paths()}>
        <p style={{ color: '#888' }}>Computing retaining paths...</p>
      </Show>
    </div>
  );
}
