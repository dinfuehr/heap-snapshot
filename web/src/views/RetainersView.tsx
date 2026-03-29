import { createSignal, createEffect, Show, For, type JSX } from 'solid-js';
import type {
  NodeInfo,
  Retainers,
  Retainer,
  RetainingPaths,
  RetainingPath,
} from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import type { NavigateOptions } from '../components/ObjectLink.tsx';
import {
  TreeTableShell,
  TreeTableRow,
  type RowSelection,
} from '../components/TreeTable.tsx';
import { TreeTablePager } from '../components/TreeTablePager.tsx';
import { formatBytes } from '../components/format.ts';

const PAGE_SIZE = 100;

function PathNode(props: {
  path: RetainingPath;
  depth: number;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
  selection: () => RowSelection | null;
  onSelect: (sel: RowSelection) => void;
}): JSX.Element {
  return (
    <TreeTableRow
      depth={props.depth}
      label={
        <>
          <span style={{ color: '#888' }}>[{props.path.edge_name}]</span>{' '}
          {props.path.node.name}{' '}
          <span style={{ color: '#888' }}>({props.path.node.node_type})</span>
        </>
      }
      linkId={props.path.node.id}
      onNavigate={props.onNavigate}
      onContextMenu={props.onContextMenu}
      selection={props.selection()}
      onSelect={props.onSelect}
      detachedness={props.path.node.detachedness}
      distance={props.path.node.distance}
      selfSize={props.path.node.self_size}
      retainedSize={props.path.node.retained_size}
    >
      <For each={props.path.children}>
        {(child) => (
          <PathNode
            path={child}
            depth={props.depth + 1}
            onNavigate={props.onNavigate}
            onContextMenu={props.onContextMenu}
            selection={props.selection}
            onSelect={props.onSelect}
          />
        )}
      </For>
    </TreeTableRow>
  );
}

function RetainerRow(props: {
  retainer: Retainer;
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
  selection: () => RowSelection | null;
  onSelect: (sel: RowSelection) => void;
  depth: number;
}): JSX.Element {
  const [expanded, setExpanded] = createSignal(false);
  const [children, setChildren] = createSignal<Retainer[] | null>(null);
  const [total, setTotal] = createSignal(0);
  const [offset, setOffset] = createSignal(0);
  const [limit, setLimit] = createSignal(PAGE_SIZE);
  const [filter, setFilter] = createSignal('');

  const loadRetainers = async (o: number, l: number, f: string) => {
    const result = await props.call<Retainers>({
      type: 'getRetainers',
      nodeId: props.retainer.source.id,
      offset: o,
      limit: l,
      filter: f,
    });
    setChildren(result.retainers);
    setTotal(result.total);
    setOffset(o);
    setLimit(l);
    setFilter(f);
  };

  const toggle = async () => {
    if (expanded()) {
      setExpanded(false);
      return;
    }
    setExpanded(true);
    if (!children()) {
      await loadRetainers(0, PAGE_SIZE, '');
    }
  };

  return (
    <TreeTableRow
      depth={props.depth}
      expanded={expanded()}
      onToggle={toggle}
      label={
        <>
          <span style={{ color: '#888' }}>[{props.retainer.edge_name}]</span>
          {' in '}
          {props.retainer.source.name}{' '}
          <span style={{ color: '#888' }}>
            ({props.retainer.source.node_type})
          </span>
        </>
      }
      linkId={props.retainer.source.id}
      onNavigate={props.onNavigate}
      onContextMenu={props.onContextMenu}
      selection={props.selection()}
      onSelect={props.onSelect}
      detachedness={props.retainer.source.detachedness}
      distance={props.retainer.source.distance}
      selfSize={props.retainer.source.self_size}
      retainedSize={props.retainer.source.retained_size}
    >
      <Show when={expanded() && children()}>
        <For each={children()!}>
          {(r) => (
            <RetainerRow
              retainer={r}
              call={props.call}
              onNavigate={props.onNavigate}
              onContextMenu={props.onContextMenu}
              selection={props.selection}
              onSelect={props.onSelect}
              depth={props.depth + 1}
            />
          )}
        </For>
        <TreeTablePager
          depth={props.depth + 1}
          shown={children()!.length}
          total={total()}
          offset={offset()}
          filter={filter()}
          onPageChange={(o, l) => loadRetainers(o, l, filter())}
          onFilterChange={(f) => loadRetainers(0, limit(), f)}
          onShowAll={() => loadRetainers(0, 999999, filter())}
        />
      </Show>
    </TreeTableRow>
  );
}

export function RetainersView(props: {
  call: SnapshotCall;
  nodeId: number | null;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
}): JSX.Element {
  const [nodeInfo, setNodeInfo] = createSignal<NodeInfo | null>(null);
  const [retainers, setRetainers] = createSignal<Retainers | null>(null);
  const [retOffset, setRetOffset] = createSignal(0);
  const [retLimit, setRetLimit] = createSignal(PAGE_SIZE);
  const [retFilter, setRetFilter] = createSignal('');
  const [paths, setPaths] = createSignal<RetainingPaths | null>(null);
  const [inputId, setInputId] = createSignal(
    props.nodeId ? `@${props.nodeId}` : '',
  );
  const [activeId, setActiveId] = createSignal<number | null>(props.nodeId);
  const [selection, setSelection] = createSignal<RowSelection | null>(null);

  const loadRetainers = async (id: number, o: number, l: number, f: string) => {
    const result = await props.call<Retainers>({
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
  };

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
    setRetainers(null);
    setNodeInfo(null);
    props.call<NodeInfo>({ type: 'getNodeInfo', nodeId: id }).then(setNodeInfo);
    loadRetainers(id, 0, PAGE_SIZE, '');
  });

  const handleSubmit = (e: Event) => {
    e.preventDefault();
    const raw = inputId().replace(/^@/, '');
    const id = parseInt(raw, 10);
    if (!isNaN(id)) setActiveId(id);
  };

  const loadPaths = async () => {
    const id = activeId();
    if (id === null) return;
    const result = await props.call<RetainingPaths>({
      type: 'getRetainingPaths',
      nodeId: id,
      maxDepth: 50,
      maxNodes: 200,
    });
    setPaths(result);
  };

  return (
    <div>
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

      <Show when={retainers() && activeId() !== null}>
        <h3 style={{ 'font-size': '14px', margin: '0 0 8px' }}>
          Direct Retainers ({retainers()!.total})
        </h3>
        <TreeTableShell>
          <For each={retainers()!.retainers}>
            {(r) => (
              <RetainerRow
                retainer={r}
                call={props.call}
                onNavigate={props.onNavigate}
                onContextMenu={props.onContextMenu}
                selection={selection}
                onSelect={setSelection}
                depth={0}
              />
            )}
          </For>
          <TreeTablePager
            depth={0}
            shown={retainers()!.retainers.length}
            total={retainers()!.total}
            offset={retOffset()}
            filter={retFilter()}
            onPageChange={(o, l) =>
              loadRetainers(activeId()!, o, l, retFilter())
            }
            onFilterChange={(f) => loadRetainers(activeId()!, 0, retLimit(), f)}
            onShowAll={() => loadRetainers(activeId()!, 0, 999999, retFilter())}
          />
        </TreeTableShell>
      </Show>

      <Show when={activeId() !== null}>
        <div style={{ 'margin-top': '16px' }}>
          <Show
            when={paths()}
            fallback={
              <button
                onClick={loadPaths}
                style={{ padding: '4px 12px', 'font-size': '14px' }}
              >
                Find retaining paths to GC roots
              </button>
            }
          >
            {(p) => (
              <>
                <h3 style={{ 'font-size': '14px', margin: '0 0 8px' }}>
                  Retaining Paths to GC Roots
                  {p().truncated && ' (truncated)'}
                  {!p().reached_gc_roots && ' (GC roots not reached)'}
                </h3>
                <TreeTableShell>
                  <For each={p().paths}>
                    {(path) => (
                      <PathNode
                        path={path}
                        depth={0}
                        onNavigate={props.onNavigate}
                        onContextMenu={props.onContextMenu}
                        selection={selection}
                        onSelect={setSelection}
                      />
                    )}
                  </For>
                </TreeTableShell>
              </>
            )}
          </Show>
        </div>
      </Show>
    </div>
  );
}
