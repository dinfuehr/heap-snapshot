import {
  createSignal,
  createResource,
  createMemo,
  Show,
  For,
  type JSX,
} from 'solid-js';
import type { AggregateEntry, SummaryExpanded, Children } from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import type { NavigateOptions } from '../components/ObjectLink.tsx';
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

function ExpandableObject(props: {
  obj: {
    id: number;
    name: string;
    self_size: number;
    retained_size: number;
    detachedness: number;
  };
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
  selection: () => RowSelection | null;
  onSelect: (sel: RowSelection) => void;
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
      selfSize={props.obj.self_size}
      retainedSize={props.obj.retained_size}
    >
      <Show when={expanded()}>
        <ObjectChildren
          nodeId={props.obj.id}
          call={props.call}
          onNavigate={props.onNavigate}
          onContextMenu={props.onContextMenu}
          selection={props.selection}
          onSelect={props.onSelect}
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
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
  selection: () => RowSelection | null;
  onSelect: (sel: RowSelection) => void;
  depth: number;
  onLoaded?: () => void;
}): JSX.Element {
  const [children, setChildren] = createSignal<
    {
      edgeLabel: string;
      node: {
        id: number;
        name: string;
        node_type: string;
        self_size: number;
        retained_size: number;
        distance: number;
        edge_count: number;
        detachedness: number;
      };
    }[]
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
            node={child.node}
            call={props.call}
            onNavigate={props.onNavigate}
            onContextMenu={props.onContextMenu}
            selection={props.selection}
            onSelect={props.onSelect}
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
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
  selection: () => RowSelection | null;
  onSelect: (sel: RowSelection) => void;
}): JSX.Element {
  const [expanded, setExpanded] = createSignal(false);
  const [loading, setLoading] = createSignal(false);
  const [objects, setObjects] = createSignal<SummaryExpanded | null>(null);
  const [objOffset, setObjOffset] = createSignal(0);

  const loadObjects = async (o: number, l: number) => {
    const result = await props.call<SummaryExpanded>({
      type: 'getSummaryObjects',
      constructor: props.entry.key,
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

  return (
    <>
      <tr
        onClick={toggle}
        onDblClick={toggle}
        style={{
          cursor: 'pointer',
          'user-select': 'none',
          background: expanded() ? '#f0f0f0' : undefined,
        }}
      >
        <td
          style={{
            padding: '2px 8px',
            overflow: 'hidden',
            'text-overflow': 'ellipsis',
            'white-space': 'nowrap',
            'max-width': '0',
          }}
        >
          {expanded()
            ? loading()
              ? '\u22EF'
              : '\u25bc'
            : '\u25b6'}{' '}
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
                />
              )}
            </For>
            <tr>
              <td colSpan={6}>
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
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
}): JSX.Element {
  const [selection, setSelection] = createSignal<RowSelection | null>(null);

  return (
    <table
      style={{
        'border-collapse': 'collapse',
        width: '100%',
        'font-size': '13px',
        'table-layout': 'fixed',
      }}
    >
      <colgroup>
        <col />
        <col style={{ width: '70px' }} />
        <col style={{ width: '90px' }} />
        <col style={{ width: '100px' }} />
        <col style={{ width: '110px' }} />
        <col style={{ width: '75px' }} />
      </colgroup>
      <thead>
        <tr
          style={{
            'text-align': 'left',
            'border-bottom': '1px solid #ccc',
          }}
        >
          <th style={{ padding: '4px 8px' }}>Constructor</th>
          <th style={{ padding: '4px 8px', 'text-align': 'right', 'white-space': 'nowrap' }}>
            Distance
          </th>
          <th style={{ padding: '4px 8px', 'text-align': 'right', 'white-space': 'nowrap' }}>
            Shallow Size
          </th>
          <th style={{ padding: '4px 8px', 'text-align': 'right', 'white-space': 'nowrap' }}>
            Retained Size
          </th>
          <th style={{ padding: '4px 8px', 'text-align': 'right', 'white-space': 'nowrap' }}>
            Reachable Size
          </th>
          <th style={{ padding: '4px 8px', 'text-align': 'right', 'white-space': 'nowrap' }}>
            Status
          </th>
        </tr>
      </thead>
      <tbody>
        <For each={props.entries}>
          {(entry) => (
            <SummaryGroup
              entry={entry}
              call={props.call}
              onNavigate={props.onNavigate}
              onContextMenu={props.onContextMenu}
              selection={selection}
              onSelect={setSelection}
            />
          )}
        </For>
      </tbody>
    </table>
  );
}

export function SummaryView(props: {
  call: SnapshotCall;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
  highlightNodeId: number | null;
}): JSX.Element {
  const [summaryFilter, setSummaryFilter] = createSignal(0);
  const [entries] = createResource(summaryFilter, async (mode) => {
    await props.call({ type: 'setSummaryFilter', mode });
    return props.call<AggregateEntry[]>({ type: 'getSummary' });
  });
  const [filter, setFilter] = createSignal('');

  const filtered = createMemo(() => {
    const e = entries();
    if (!e) return null;
    const f = filter().toLowerCase();
    if (!f) return e;
    return e.filter((entry) => entry.name.toLowerCase().includes(f));
  });

  return (
    <div>
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
              onInput={(e) => setFilter(e.currentTarget.value)}
              placeholder="Filter constructors..."
              style={{
                padding: '4px 8px',
                'font-size': '13px',
                width: '250px',
              }}
            />
            <select
              value={summaryFilter()}
              onChange={(e) => {
                setSummaryFilter(parseInt(e.currentTarget.value, 10));
              }}
              style={{
                padding: '4px 8px',
                'font-size': '13px',
              }}
            >
              <option value={0}>All objects</option>
              <option value={1}>Unreachable (all)</option>
              <option value={2}>Unreachable (roots only)</option>
              <option value={3}>Retained by detached DOM</option>
              <option value={4}>Retained by DevTools console</option>
              <option value={5}>Retained by event handlers</option>
            </select>
            <Show when={entries.loading}>
              <span style={{ 'font-size': '12px', color: '#888' }}>Loading...</span>
            </Show>
          </div>
          <Show when={filtered()} fallback={
            <Show when={!entries.loading}>
              <p>Loading...</p>
            </Show>
          }>
            {(list) => (
              <>
                <div style={{ 'margin-bottom': '4px', 'font-size': '12px', color: '#888', display: 'flex', gap: '8px' }}>
                  <Show when={filter()}>
                    <span>{list().length} of {entries()!.length} groups</span>
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
                />
              </>
            )}
          </Show>
        </div>
  );
}
