import { createSignal, Show, For, type JSX } from 'solid-js';
import type { NodeInfo, Children, ReachableSizeInfo } from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import type { NavigateOptions } from '../components/ObjectLink.tsx';
import {
  TreeTableShell,
  TreeTableRow,
  type RowSelection,
} from '../components/TreeTable.tsx';
import { TreeTablePager } from '../components/TreeTablePager.tsx';
import { TreeTableLoading } from '../components/TreeTable.tsx';

const PAGE_SIZE = 100;

function HistoryEntry(props: {
  node: NodeInfo;
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
  selection: () => RowSelection | null;
  onSelect: (sel: RowSelection) => void;
  reachableSizes: Map<number, ReachableSizeInfo>;
}): JSX.Element {
  const [expanded, setExpanded] = createSignal(false);
  const [children, setChildren] = createSignal<
    { edgeType: string; edgeName: string; node: NodeInfo }[] | null
  >(null);
  const [total, setTotal] = createSignal(0);
  const [offset, setOffset] = createSignal(0);
  const [limit, setLimit] = createSignal(PAGE_SIZE);
  const [filter, setFilter] = createSignal('');

  const loadChildren = async (o: number, l: number, f: string) => {
    const result = await props.call<Children>({
      type: 'getChildren',
      nodeId: props.node.id,
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
  };

  const toggle = async () => {
    if (expanded()) {
      setExpanded(false);
      return;
    }
    setExpanded(true);
    if (!children()) {
      await loadChildren(0, PAGE_SIZE, '');
    }
  };

  const hasChildren = props.node.edge_count > 0;

  return (
    <TreeTableRow
      depth={0}
      expanded={expanded()}
      onToggle={hasChildren ? toggle : undefined}
      label={
        <>
          {props.node.name}{' '}
          <span style={{ color: '#888' }}>({props.node.node_type})</span>
        </>
      }
      linkId={props.node.id}
      onNavigate={props.onNavigate}
      onContextMenu={props.onContextMenu}
      selection={props.selection()}
      onSelect={props.onSelect}
      detachedness={props.node.detachedness}
      distance={props.node.distance}
      selfSize={props.node.self_size}
      retainedSize={props.node.retained_size}
      reachableInfo={props.reachableSizes.get(props.node.id)}
    >
      <Show when={expanded() && !children()}>
        <TreeTableLoading depth={1} />
      </Show>
      <Show when={expanded() && children()}>
        <For each={children()!}>
          {(child) => (
            <TreeTableRow
              depth={1}
              prefix={
                <span style={{ color: '#888' }}>
                  {child.edgeType === 'element' || child.edgeType === 'hidden'
                    ? `[${child.edgeName}] :: `
                    : `${child.edgeName} :: `}
                </span>
              }
              label={<>{child.node.name}</>}
              linkId={child.node.id}
              onNavigate={props.onNavigate}
              onContextMenu={props.onContextMenu}
              selection={props.selection()}
              onSelect={props.onSelect}
              detachedness={child.node.detachedness}
              distance={child.node.distance}
              selfSize={child.node.self_size}
              retainedSize={child.node.retained_size}
              reachableInfo={props.reachableSizes.get(child.node.id)}
            />
          )}
        </For>
        <TreeTablePager
          depth={1}
          shown={children()!.length}
          total={total()}
          offset={offset()}
          filter={filter()}
          onPageChange={(o, l) => loadChildren(o, l, filter())}
          onFilterChange={(f) => loadChildren(0, limit(), f)}
          onShowAll={() => loadChildren(0, 999999, filter())}
        />
      </Show>
    </TreeTableRow>
  );
}

export function HistoryView(props: {
  call: SnapshotCall;
  history: NodeInfo[];
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
  reachableSizes: Map<number, ReachableSizeInfo>;
}): JSX.Element {
  const [selection, setSelection] = createSignal<RowSelection | null>(null);

  return (
    <Show
      when={props.history.length > 0}
      fallback={
        <p style={{ color: '#888', 'font-size': '13px' }}>
          No history yet. Navigate to objects via the Retainers view or
          right-click context menu.
        </p>
      }
    >
      <TreeTableShell>
        <For each={[...props.history].reverse()}>
          {(node) => (
            <HistoryEntry
              node={node}
              call={props.call}
              onNavigate={props.onNavigate}
              onContextMenu={props.onContextMenu}
              selection={selection}
              onSelect={setSelection}
              reachableSizes={props.reachableSizes}
            />
          )}
        </For>
      </TreeTableShell>
    </Show>
  );
}
