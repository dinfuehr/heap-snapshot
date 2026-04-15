import {
  createSignal,
  createResource,
  createEffect,
  Show,
  For,
  type JSX,
} from 'solid-js';
import type {
  NativeContext,
  Children,
  NodeInfo,
  ReachableSizeInfo,
} from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import type { NavigateOptions } from '../components/ObjectLink.tsx';
import {
  TreeTableShell,
  TreeTableRow,
  type RowSelection,
} from '../components/TreeTable.tsx';
import { TreeTablePager } from '../components/TreeTablePager.tsx';
import { TreeTableLoading } from '../components/TreeTable.tsx';
import { ContainmentTreeNode } from './ContainmentView.tsx';

const PAGE_SIZE = 100;

function ContextNode(props: {
  ctx: NativeContext;
  index: number;
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
  selection: () => RowSelection | null;
  onSelect: (sel: RowSelection) => void;
  reachableSizes: Map<number, ReachableSizeInfo>;
  reachablePending: Set<number>;
}): JSX.Element {
  const [expanded, setExpanded] = createSignal(false);
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
      nodeId: props.ctx.id,
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

  return (
    <TreeTableRow
      depth={0}
      expanded={expanded()}
      onToggle={toggle}
      label={
        <>
          {props.ctx.label}
          <Show when={props.ctx.vars}>
            <span
              style={{
                color: '#888',
                'font-size': '11px',
                'margin-left': '8px',
              }}
            >
              Vars: {props.ctx.vars}
            </span>
          </Show>
        </>
      }
      linkId={props.ctx.id}
      onNavigate={props.onNavigate}
      onContextMenu={props.onContextMenu}
      selection={props.selection()}
      onSelect={props.onSelect}
      detachedness={
        props.ctx.detachedness === 'detached'
          ? 2
          : props.ctx.detachedness === 'attached'
            ? 1
            : 0
      }
      ctx={`#${props.index}`}
      ctxLabel={props.ctx.label}
      selfSize={props.ctx.self_size}
      retainedSize={props.ctx.retained_size}
      reachableInfo={props.reachableSizes.get(props.ctx.id)}
      reachableLoading={props.reachablePending.has(props.ctx.id)}
    >
      <Show when={expanded() && !children()}>
        <TreeTableLoading depth={1} />
      </Show>
      <Show when={expanded() && children()}>
        <For each={children()!}>
          {(child) => (
            <ContainmentTreeNode
              edgeLabel={child.edgeLabel}
              node={child.node}
              call={props.call}
              onNavigate={props.onNavigate}
              onContextMenu={props.onContextMenu}
              selection={props.selection}
              onSelect={props.onSelect}
              reachableSizes={props.reachableSizes}
              reachablePending={props.reachablePending}
              depth={1}
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

export function ContextsView(props: {
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
  reachableSizes: Map<number, ReachableSizeInfo>;
  reachablePending: Set<number>;
  onReachableSize: (nodeId: number, info: ReachableSizeInfo) => void;
  onMarkPending: (nodeId: number) => void;
}): JSX.Element {
  const [contexts] = createResource(() =>
    props.call<NativeContext[]>({ type: 'getNativeContexts' }),
  );
  const [selection, setSelection] = createSignal<RowSelection | null>(null);

  // Auto-compute reachable sizes for all native contexts, one at a time
  // so other worker requests can interleave between them.
  const queued = new Set<number>();
  createEffect(() => {
    const ctxs = contexts();
    if (!ctxs) return;
    (async () => {
      for (const ctx of ctxs) {
        if (queued.has(ctx.id)) continue;
        queued.add(ctx.id);
        props.onMarkPending(ctx.id);
        const info = await props.call<ReachableSizeInfo>(
          { type: 'getReachableSize', nodeId: ctx.id },
          { background: true },
        );
        props.onReachableSize(ctx.id, info);
      }
    })();
  });

  return (
    <Show when={contexts()} fallback={<p>Loading...</p>}>
      {(ctxs) => (
        <Show
          when={ctxs().length > 0}
          fallback={<p>No native contexts found.</p>}
        >
          <div class="tab-panel">
            <p
              style={{ 'font-size': '13px', color: '#888', margin: '0 0 8px' }}
            >
              {ctxs().length} native context{ctxs().length !== 1 ? 's' : ''}{' '}
              (JavaScript realms)
            </p>
            <TreeTableShell>
              <For each={ctxs()}>
                {(ctx, i) => (
                  <ContextNode
                    ctx={ctx}
                    index={i()}
                    call={props.call}
                    onNavigate={props.onNavigate}
                    onContextMenu={props.onContextMenu}
                    selection={selection}
                    onSelect={setSelection}
                    reachableSizes={props.reachableSizes}
                    reachablePending={props.reachablePending}
                  />
                )}
              </For>
            </TreeTableShell>
          </div>
        </Show>
      )}
    </Show>
  );
}
