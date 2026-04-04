import { createSignal, createResource, Show, For, type JSX } from 'solid-js';
import type { Containment, Children, NodeInfo } from '../types.ts';
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

export { TreeNode as ContainmentTreeNode };

function TreeNode(props: {
  edgeLabel?: string;
  node: NodeInfo;
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
  selection: () => RowSelection | null;
  onSelect: (sel: RowSelection) => void;
  depth: number;
  initialExpanded?: boolean;
}): JSX.Element {
  const [expanded, setExpanded] = createSignal(props.initialExpanded ?? false);
  const [loading, setLoading] = createSignal(false);
  const [children, setChildren] = createSignal<
    { edgeLabel: string; node: NodeInfo }[] | null
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
        edgeLabel:
          e.edge_type === 'element' || e.edge_type === 'hidden'
            ? `[${e.edge_name}] :: `
            : `${e.edge_name} :: `,
        node: e.target,
      })),
    );
    setTotal(result.total);
    setOffset(o);
    setLimit(l);
    setFilter(f);
  };

  if (props.initialExpanded) {
    loadChildren(0, PAGE_SIZE, '');
  }

  const toggle = async () => {
    if (expanded()) {
      setExpanded(false);
      return;
    }
    if (!children()) {
      setLoading(true);
      await loadChildren(0, PAGE_SIZE, '');
      setLoading(false);
    }
    setExpanded(true);
  };

  const hasChildren = props.node.edge_count > 0;

  return (
    <TreeTableRow
      depth={props.depth}
      expanded={expanded()}
      loading={loading()}
      onToggle={hasChildren ? toggle : undefined}
      prefix={
        props.edgeLabel ? (
          <span style={{ color: '#888' }}>{props.edgeLabel}</span>
        ) : undefined
      }
      label={<>{props.node.name}</>}
      linkId={props.node.id}
      onNavigate={props.onNavigate}
      onContextMenu={props.onContextMenu}
      selection={props.selection()}
      onSelect={props.onSelect}
      detachedness={props.node.detachedness}
      distance={props.node.distance}
      selfSize={props.node.self_size}
      retainedSize={props.node.retained_size}
    >
      <Show when={expanded() && !children()}>
        <TreeTableLoading depth={props.depth + 1} />
      </Show>
      <Show when={expanded() && children()}>
        <For each={children()!}>
          {(child) => (
            <TreeNode
              edgeLabel={child.edgeLabel}
              node={child.node}
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
          onPageChange={(o, l) => loadChildren(o, l, filter())}
          onFilterChange={(f) => loadChildren(0, limit(), f)}
          onShowAll={() => loadChildren(0, 999999, filter())}
        />
      </Show>
    </TreeTableRow>
  );
}

export function ContainmentView(props: {
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
}): JSX.Element {
  const [containment] = createResource(() =>
    props.call<Containment>({ type: 'getContainment' }),
  );
  const [selection, setSelection] = createSignal<RowSelection | null>(null);

  return (
    <Show when={containment()} fallback={<p>Loading...</p>}>
      <TreeTableShell>
        <For each={containment()!.system_roots}>
          {(edge) => (
            <TreeNode
              edgeLabel={
                edge.edge_type === 'element' || edge.edge_type === 'hidden'
                  ? `[${edge.edge_name}] :: `
                  : `${edge.edge_name} :: `
              }
              node={edge.target}
              call={props.call}
              onNavigate={props.onNavigate}
              onContextMenu={props.onContextMenu}
              selection={selection}
              onSelect={setSelection}
              depth={0}
              initialExpanded={edge.target.name === '(GC roots)'}
            />
          )}
        </For>
      </TreeTableShell>
    </Show>
  );
}
