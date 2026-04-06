import { createSignal, createResource, Show, For, type JSX } from 'solid-js';
import type { NodeInfo, DominatedChildren, ReachableSizeInfo } from '../types.ts';
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

function DomTreeNode(props: {
  node: NodeInfo;
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
  selection: () => RowSelection | null;
  onSelect: (sel: RowSelection) => void;
  reachableSizes: Map<number, ReachableSizeInfo>;
  depth: number;
}): JSX.Element {
  const [expanded, setExpanded] = createSignal(false);
  const [children, setChildren] = createSignal<NodeInfo[] | null>(null);
  const [total, setTotal] = createSignal(0);
  const [offset, setOffset] = createSignal(0);

  const loadChildren = async (o: number, l: number) => {
    const result = await props.call<DominatedChildren>({
      type: 'getDominatedChildren',
      nodeId: props.node.id,
      offset: o,
      limit: l,
    });
    setChildren(result.children);
    setTotal(result.total);
    setOffset(o);
  };

  const toggle = async () => {
    if (expanded()) {
      setExpanded(false);
      return;
    }
    setExpanded(true);
    if (!children()) {
      await loadChildren(0, PAGE_SIZE);
    }
  };

  return (
    <TreeTableRow
      depth={props.depth}
      expanded={expanded()}
      onToggle={toggle}
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
        <TreeTableLoading depth={props.depth + 1} />
      </Show>
      <Show when={expanded() && children()}>
        <For each={children()!}>
          {(child) => (
            <DomTreeNode
              node={child}
              call={props.call}
              onNavigate={props.onNavigate}
              onContextMenu={props.onContextMenu}
              selection={props.selection}
              onSelect={props.onSelect}
              reachableSizes={props.reachableSizes}
              depth={props.depth + 1}
            />
          )}
        </For>
        <TreeTablePager
          depth={props.depth + 1}
          shown={children()!.length}
          total={total()}
          offset={offset()}
          filter=""
          onPageChange={(o, l) => loadChildren(o, l)}
          onFilterChange={() => {}}
          onShowAll={() => loadChildren(0, 999999)}
        />
      </Show>
    </TreeTableRow>
  );
}

export function DominatorsView(props: {
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
  reachableSizes: Map<number, ReachableSizeInfo>;
  focusNodeId: number | null;
}): JSX.Element {
  const [root] = createResource(() =>
    props.call<NodeInfo>({ type: 'getDominatorTreeRoot' }),
  );
  const [selection, setSelection] = createSignal<RowSelection | null>(null);

  return (
    <Show when={root()} fallback={<p>Loading...</p>}>
      <TreeTableShell>
        <DomTreeNode
          node={root()!}
          call={props.call}
          onNavigate={props.onNavigate}
          onContextMenu={props.onContextMenu}
          selection={selection}
          onSelect={setSelection}
          reachableSizes={props.reachableSizes}
          depth={0}
        />
      </TreeTableShell>
    </Show>
  );
}
