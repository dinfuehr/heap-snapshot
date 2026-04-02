import { createSignal, createEffect, Show, For, type JSX } from 'solid-js';
import type { NodeInfo, RetainingPaths, RetainingPath } from '../types.ts';
import type { SnapshotCall } from '../worker/use-snapshot.ts';
import type { NavigateOptions } from '../components/ObjectLink.tsx';
import {
  TreeTableShell,
  TreeTableRow,
  type RowSelection,
} from '../components/TreeTable.tsx';
import { formatBytes } from '../components/format.ts';

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
      prefix={
        <span style={{ color: '#888' }}>
          {props.path.edge_type === 'element' || props.path.edge_type === 'hidden'
            ? `[${props.path.edge_name}] in `
            : `${props.path.edge_name} in `}
        </span>
      }
      label={<>{props.path.node.name}</>}
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

export function RetainersView(props: {
  call: SnapshotCall;
  nodeId: number | null;
  onNavigate: (opts: NavigateOptions) => void;
  onContextMenu: (e: MouseEvent, nodeId: number) => void;
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

      <Show when={paths()}>
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

      <Show when={activeId() !== null && !paths()}>
        <p style={{ color: '#888' }}>Computing retaining paths...</p>
      </Show>
    </div>
  );
}
