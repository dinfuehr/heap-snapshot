import {
  createSignal,
  createResource,
  createMemo,
  createEffect,
  on,
  Show,
  For,
  untrack,
  type JSX,
} from 'solid-js';
import type {
  AggregateEntry,
  SummaryExpanded,
  Children,
  NodeInfo,
  ReachableSizeInfo,
  NativeContext,
} from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import type { NavigateOptions, EdgeInfo } from '../components/ObjectLink.tsx';
import { formatBytes } from '../components/format.ts';
import { TreeTablePager } from '../components/TreeTablePager.tsx';
import {
  TreeTableRow,
  TreeTableLoading,
  type RowSelection,
} from '../components/TreeTable.tsx';
import { ContainmentTreeNode } from './ContainmentView.tsx';

const numTd = {
  padding: '2px 8px',
  'text-align': 'right' as const,
  'font-variant-numeric': 'tabular-nums',
};

const PAGE_SIZE = 100;

interface FocusTarget {
  nodeId: number;
  constructorIndex: number;
  pageOffset: number;
}

function ExpandableObject(props: {
  obj: {
    id: number;
    name: string;
    self_size: number;
    retained_size: number;
    detachedness: number;
    ctx: string;
    ctx_label: string;
  };
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number, edgeInfo?: EdgeInfo) => void;
  selection: () => RowSelection | null;
  onSelect: (sel: RowSelection) => void;
  reachableSizes: Map<number, ReachableSizeInfo>;
  reachablePending: Set<number>;
}): JSX.Element {
  const [expanded, setExpanded] = createSignal(false);
  const [loading, setLoading] = createSignal(false);
  const [childrenLoaded, setChildrenLoaded] = createSignal(false);

  const toggle = async () => {
    if (expanded()) {
      setExpanded(false);
      return;
    }
    if (!childrenLoaded()) {
      setLoading(true);
      setExpanded(true);
      return;
    }
    setExpanded(true);
  };

  return (
    <TreeTableRow
      depth={1}
      expanded={expanded() && !loading()}
      loading={loading()}
      onToggle={toggle}
      label={<>{props.obj.name}</>}
      linkId={props.obj.id}
      onNavigate={props.onNavigate}
      onContextMenu={props.onContextMenu}
      selection={props.selection()}
      onSelect={props.onSelect}
      detachedness={props.obj.detachedness}
      ctx={props.obj.ctx}
      ctxLabel={props.obj.ctx_label}
      selfSize={props.obj.self_size}
      retainedSize={props.obj.retained_size}
      reachableInfo={props.reachableSizes.get(props.obj.id)}
      reachableLoading={props.reachablePending.has(props.obj.id)}
    >
      <Show when={expanded()}>
        <ObjectChildren
          nodeId={props.obj.id}
          call={props.call}
          onNavigate={props.onNavigate}
          onContextMenu={props.onContextMenu}
          selection={props.selection}
          onSelect={props.onSelect}
          reachableSizes={props.reachableSizes}
          reachablePending={props.reachablePending}
          depth={2}
          onLoaded={() => {
            setChildrenLoaded(true);
            setLoading(false);
          }}
        />
      </Show>
    </TreeTableRow>
  );
}

function ObjectChildren(props: {
  nodeId: number;
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number, edgeInfo?: EdgeInfo) => void;
  selection: () => RowSelection | null;
  onSelect: (sel: RowSelection) => void;
  reachableSizes: Map<number, ReachableSizeInfo>;
  reachablePending: Set<number>;
  depth: number;
  onLoaded?: () => void;
}): JSX.Element {
  const [children, setChildren] = createSignal<
    { edgeLabel: string; edgeInfo: EdgeInfo; node: NodeInfo }[]
  >([]);
  const [loaded, setLoaded] = createSignal(false);
  const [total, setTotal] = createSignal(0);
  const [offset, setOffset] = createSignal(0);
  const [filter, setFilter] = createSignal('');

  const loadChildren = async (o: number, l: number, f: string) => {
    const result = await props.call<Children>({
      type: 'getChildren',
      nodeId: props.nodeId,
      offset: o,
      limit: l,
      filter: f,
    });
    setChildren(
      result.edges.map((e) => ({
        edgeLabel:
          e.edge_type === 'element' || e.edge_type === 'hidden'
            ? `[${e.edge_name}] :: `
            : `${e.edge_name} :: `,
        edgeInfo: {
          edgeType: e.edge_type,
          edgeName: e.edge_name,
          parentId: props.nodeId,
        },
        node: e.target,
      })),
    );
    setTotal(result.total);
    setOffset(o);
    setFilter(f);
    setLoaded(true);
    props.onLoaded?.();
  };

  loadChildren(0, PAGE_SIZE, '');

  return (
    <Show when={loaded()} fallback={<TreeTableLoading depth={props.depth} />}>
      <For each={children()}>
        {(child) => (
          <ContainmentTreeNode
            edgeLabel={child.edgeLabel}
            edgeInfo={child.edgeInfo}
            node={child.node}
            call={props.call}
            onNavigate={props.onNavigate}
            onContextMenu={props.onContextMenu}
            selection={props.selection}
            onSelect={props.onSelect}
            reachableSizes={props.reachableSizes}
            reachablePending={props.reachablePending}
            depth={props.depth}
          />
        )}
      </For>
      <TreeTablePager
        depth={props.depth}
        shown={children().length}
        total={total()}
        offset={offset()}
        filter={filter()}
        onPageChange={(o, l) => loadChildren(o, l, filter())}
        onFilterChange={(f) => loadChildren(0, PAGE_SIZE, f)}
        onShowAll={() => loadChildren(0, 999999, filter())}
      />
    </Show>
  );
}

function SummaryGroup(props: {
  entry: AggregateEntry;
  call: SnapshotCall;
  objectsMessageType?: string;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number, edgeInfo?: EdgeInfo) => void;
  selection: () => RowSelection | null;
  onSelect: (sel: RowSelection) => void;
  reachableSizes: Map<number, ReachableSizeInfo>;
  reachablePending: Set<number>;
  focusTarget: () => FocusTarget | null;
  onFocusHandled: () => void;
  containerRef: () => HTMLElement | undefined;
}): JSX.Element {
  const [expanded, setExpanded] = createSignal(false);
  const [loading, setLoading] = createSignal(false);
  const [objects, setObjects] = createSignal<SummaryExpanded | null>(null);
  const [objOffset, setObjOffset] = createSignal(0);

  const loadObjects = async (o: number, l: number) => {
    const msgType = props.objectsMessageType ?? 'getSummaryObjects';
    const result = await props.call<SummaryExpanded>({
      type: msgType as 'getSummaryObjects',
      constructorIndex: props.entry.index,
      offset: o,
      limit: l,
    });
    setObjects(result);
    setObjOffset(o);
  };

  const toggle = async () => {
    if (expanded()) {
      setExpanded(false);
      return;
    }
    setExpanded(true);
    setLoading(true);
    setObjects(null);
    await loadObjects(0, 100);
    setLoading(false);
  };

  // Auto-expand and scroll to the focused object when focusTarget matches.
  createEffect(
    on(props.focusTarget, (target) => {
      if (!target || target.constructorIndex !== props.entry.index) return;
      const cur = untrack(objOffset);
      const isLoaded = untrack(expanded) && cur === target.pageOffset;
      if (isLoaded) {
        // Already showing the right page — just scroll.
        requestAnimationFrame(() => {
          scrollToNode(target.nodeId);
          props.onFocusHandled();
        });
        return;
      }
      setExpanded(true);
      setLoading(true);
      setObjects(null);
      loadObjects(target.pageOffset, PAGE_SIZE).then(() => {
        setLoading(false);
        requestAnimationFrame(() => {
          scrollToNode(target.nodeId);
          props.onFocusHandled();
        });
      });
    }),
  );

  const scrollToNode = (nodeId: number) => {
    const container = props.containerRef();
    const el = container
      ? container.querySelector(`tr[data-node-id="${nodeId}"]`)
      : document.querySelector(`tr[data-node-id="${nodeId}"]`);
    el?.scrollIntoView({ block: 'center' });
    props.onSelect({ rowId: -1, nodeId });
  };

  return (
    <>
      <tr
        onClick={toggle}
        onDblClick={toggle}
        style={{
          cursor: 'pointer',
          background: expanded() ? '#f0f0f0' : undefined,
        }}
      >
        <td
          style={{
            padding: '2px 8px',
            'white-space': 'nowrap',
            overflow: 'hidden',
            'text-overflow': 'ellipsis',
          }}
        >
          <span style={{ 'user-select': 'none' }}>
            {expanded() ? (loading() ? '\u22EF' : '\u25bc') : '\u25b6'}{' '}
          </span>
          {props.entry.name}{' '}
          <span style={{ color: '#888' }}>
            {'\u00d7'}
            {props.entry.count.toLocaleString()}
          </span>
        </td>
        <td style={numTd} />
        <td style={numTd}>{formatBytes(props.entry.self_size)}</td>
        <td style={numTd}>{formatBytes(props.entry.retained_size)}</td>
        <td style={{ ...numTd, color: '#ccc' }}>{'\u2014'}</td>
        <td />
        <td />
      </tr>
      <Show when={expanded() && objects()}>
        {(objs) => (
          <>
            <For each={objs().objects}>
              {(obj) => (
                <ExpandableObject
                  obj={obj}
                  call={props.call}
                  onNavigate={props.onNavigate}
                  onContextMenu={props.onContextMenu}
                  selection={props.selection}
                  onSelect={props.onSelect}
                  reachableSizes={props.reachableSizes}
                  reachablePending={props.reachablePending}
                />
              )}
            </For>
            <tr>
              <td colSpan={7}>
                <TreeTablePager
                  depth={1}
                  shown={objs().objects.length}
                  total={objs().total}
                  offset={objOffset()}
                  filter=""
                  onPageChange={(o, l) => loadObjects(o, l)}
                  onFilterChange={() => {}}
                  onShowAll={() => loadObjects(0, 999999)}
                />
              </td>
            </tr>
          </>
        )}
      </Show>
    </>
  );
}

export function SummaryTable(props: {
  entries: AggregateEntry[];
  call: SnapshotCall;
  objectsMessageType?: string;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number, edgeInfo?: EdgeInfo) => void;
  reachableSizes: Map<number, ReachableSizeInfo>;
  reachablePending: Set<number>;
  focusTarget?: () => FocusTarget | null;
  onFocusHandled?: () => void;
}): JSX.Element {
  const [selection, setSelection] = createSignal<RowSelection | null>(null);
  let containerEl: HTMLDivElement | undefined;

  return (
    <div
      ref={containerEl}
      style={{
        flex: '1',
        'min-height': '0',
        overflow: 'auto',
      }}
    >
      <table
        style={{
          'border-collapse': 'collapse',
          width: '100%',
          'table-layout': 'fixed',
          'font-size': '13px',
        }}
      >
        <colgroup>
          <col />
          <col style={{ width: '80px' }} />
          <col style={{ width: '90px' }} />
          <col style={{ width: '110px' }} />
          <col style={{ width: '120px' }} />
          <col style={{ width: '80px' }} />
          <col style={{ width: '50px' }} />
        </colgroup>
        <thead>
          <tr
            style={{
              'text-align': 'left',
              'border-bottom': '1px solid #ccc',
              background: 'white',
              position: 'sticky',
              top: '0',
              'z-index': 1,
            }}
          >
            <th style={{ padding: '4px 8px' }}>Constructor</th>
            <th
              style={{
                padding: '4px 8px',
                'text-align': 'right',
                'white-space': 'nowrap',
              }}
            >
              Distance
            </th>
            <th
              style={{
                padding: '4px 8px',
                'text-align': 'right',
                'white-space': 'nowrap',
              }}
            >
              Shallow Size
            </th>
            <th
              style={{
                padding: '4px 8px',
                'text-align': 'right',
                'white-space': 'nowrap',
              }}
            >
              Retained Size
            </th>
            <th
              style={{
                padding: '4px 8px',
                'text-align': 'right',
                'white-space': 'nowrap',
              }}
            >
              Reachable Size
            </th>
            <th
              style={{
                padding: '4px 8px',
                'text-align': 'right',
                'white-space': 'nowrap',
              }}
            >
              Status
            </th>
            <th
              style={{
                padding: '4px 8px',
                'text-align': 'right',
                'white-space': 'nowrap',
              }}
            >
              Ctx
            </th>
          </tr>
        </thead>
        <tbody>
          <For each={props.entries}>
            {(entry) => (
              <SummaryGroup
                entry={entry}
                call={props.call}
                objectsMessageType={props.objectsMessageType}
                onNavigate={props.onNavigate}
                onContextMenu={props.onContextMenu}
                selection={selection}
                onSelect={setSelection}
                reachableSizes={props.reachableSizes}
                reachablePending={props.reachablePending}
                focusTarget={props.focusTarget ?? (() => null)}
                onFocusHandled={props.onFocusHandled ?? (() => {})}
                containerRef={() => containerEl}
              />
            )}
          </For>
        </tbody>
      </table>
    </div>
  );
}

export function SummaryView(props: {
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number, edgeInfo?: EdgeInfo) => void;
  reachableSizes: Map<number, ReachableSizeInfo>;
  reachablePending: Set<number>;
  highlightNodeId: number | null;
}): JSX.Element {
  const [contexts] = createResource(() =>
    props.call<NativeContext[]>({ type: 'getNativeContexts' }),
  );
  const [summaryFilter, setSummaryFilter] = createSignal('0');
  const [entries, { refetch: refetchEntries }] = createResource(
    summaryFilter,
    async (key) => {
      if (key.startsWith('ctx:')) {
        const sub = key.slice(4);
        if (sub === 'shared') {
          return props.call<AggregateEntry[]>({
            type: 'getSummaryWithContextFilter',
            contextMode: 1,
            contextIndex: 0,
          });
        } else if (sub === 'unattributed') {
          return props.call<AggregateEntry[]>({
            type: 'getSummaryWithContextFilter',
            contextMode: 2,
            contextIndex: 0,
          });
        } else {
          return props.call<AggregateEntry[]>({
            type: 'getSummaryWithContextFilter',
            contextMode: 0,
            contextIndex: parseInt(sub, 10),
          });
        }
      } else {
        return props.call<AggregateEntry[]>({
          type: 'getSummaryWithFilter',
          mode: parseInt(key, 10),
        });
      }
    },
  );
  const [filter, setFilter] = createSignal('');
  const [searchError, setSearchError] = createSignal<string | null>(null);
  const [focusTarget, setFocusTarget] = createSignal<FocusTarget | null>(null);

  const focusOnNode = async (nodeId: number) => {
    // Switch to "All objects" and wait for entries to arrive before
    // computing indices — stale rows could match the wrong index.
    setSummaryFilter('0');
    // The resource fetcher updates the worker filter and fetches entries.
    // Await refetch to ensure rows re-render with correct indices before
    // we set the focus target.
    await refetchEntries();
    const constructorIndex = await props.call<number>({
      type: 'getConstructorForNode',
      nodeId,
    });
    const pos = await props.call<{ index: number; total: number }>({
      type: 'getSummaryObjectIndex',
      constructorIndex,
      nodeId,
    });
    const pageOffset = Math.floor(pos.index / PAGE_SIZE) * PAGE_SIZE;
    setFilter('');
    setFocusTarget({ nodeId, constructorIndex, pageOffset });
  };

  const handleFilterKeyDown = async (e: KeyboardEvent) => {
    if (e.key !== 'Enter') return;
    const value = filter().trim();
    if (!value.startsWith('@')) return;
    e.preventDefault();

    const idStr = value.slice(1);
    const id = parseInt(idStr, 10);
    if (isNaN(id) || idStr === '') {
      setSearchError(`Invalid id: ${idStr}`);
      return;
    }

    try {
      setSearchError(null);
      await focusOnNode(id);
    } catch {
      setSearchError(`No object found with id @${id}`);
    }
  };

  // React to external "Show in summary" navigation.
  createEffect(
    on(
      () => props.highlightNodeId,
      (nodeId) => {
        if (nodeId === null) return;
        focusOnNode(nodeId).catch(() => {});
      },
    ),
  );

  const filtered = createMemo(() => {
    const e = entries();
    if (!e) return null;
    const f = filter().toLowerCase();
    if (!f || f.startsWith('@')) return e;
    return e.filter((entry) => entry.name.toLowerCase().includes(f));
  });

  return (
    <div class="tab-panel">
      <div
        style={{
          'margin-bottom': '8px',
          display: 'flex',
          'align-items': 'center',
          gap: '8px',
        }}
      >
        <input
          type="text"
          value={filter()}
          onInput={(e) => {
            setFilter(e.currentTarget.value);
            setSearchError(null);
          }}
          onKeyDown={handleFilterKeyDown}
          placeholder="Filter constructors or @id..."
          style={{
            padding: '4px 8px',
            'font-size': '13px',
            width: '250px',
          }}
        />
        <select
          value={summaryFilter()}
          onChange={(e) => {
            setSummaryFilter(e.currentTarget.value);
          }}
          style={{
            padding: '4px 8px',
            'font-size': '13px',
          }}
        >
          <option value="0">All objects</option>
          <option value="6">Attached</option>
          <option value="7">Detached</option>
          <option value="1">Unreachable (all)</option>
          <option value="2">Unreachable (roots only)</option>
          <option value="3">Retained by detached DOM</option>
          <option value="4">Retained by DevTools console</option>
          <option value="5">Retained by event handlers</option>
          <Show when={contexts() && contexts()!.length > 0}>
            <optgroup label="Native contexts">
              <For each={contexts()!}>
                {(ctx, i) => <option value={`ctx:${i()}`}>{ctx.label}</option>}
              </For>
              <option value="ctx:shared">Shared (multiple contexts)</option>
              <option value="ctx:unattributed">Unattributed</option>
            </optgroup>
          </Show>
        </select>
        <Show when={entries.loading}>
          <span style={{ 'font-size': '12px', color: '#888' }}>Loading...</span>
        </Show>
        <Show when={searchError()}>
          {(err) => (
            <span style={{ 'font-size': '12px', color: '#c00' }}>{err()}</span>
          )}
        </Show>
      </div>
      <Show
        when={filtered()}
        fallback={
          <Show when={!entries.loading}>
            <p>Loading...</p>
          </Show>
        }
      >
        {(list) => (
          <>
            <div
              style={{
                'margin-bottom': '4px',
                'font-size': '12px',
                color: '#888',
                display: 'flex',
                gap: '8px',
              }}
            >
              <Show when={filter() && !filter().startsWith('@')}>
                <span>
                  {list().length} of {entries()!.length} groups
                </span>
              </Show>
              <span>
                {list()
                  .reduce((s, e) => s + e.count, 0)
                  .toLocaleString()}{' '}
                objects,{' '}
                {formatBytes(list().reduce((s, e) => s + e.self_size, 0))}{' '}
                shallow
              </span>
            </div>
            <SummaryTable
              entries={list()}
              call={props.call}
              onNavigate={props.onNavigate}
              onContextMenu={props.onContextMenu}
              reachableSizes={props.reachableSizes}
              reachablePending={props.reachablePending}
              focusTarget={focusTarget}
              onFocusHandled={() => setFocusTarget(null)}
            />
          </>
        )}
      </Show>
    </div>
  );
}
