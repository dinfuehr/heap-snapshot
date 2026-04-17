import {
  createSignal,
  Show,
  For,
  onCleanup,
  onMount,
  type JSX,
} from 'solid-js';
import { createSnapshot } from './worker/use-snapshot.ts';
import { FileLoader } from './components/FileLoader.tsx';
import { TabNav } from './components/TabNav.tsx';
import { ContextMenu } from './components/ContextMenu.tsx';
import { InspectDialog } from './components/InspectDialog.tsx';
import type { NavigateOptions, EdgeInfo } from './components/ObjectLink.tsx';
import type { ReachableSizeInfo, NodeInfo } from './types.ts';
import { StatisticsView } from './views/StatisticsView.tsx';
import { SummaryView } from './views/SummaryView.tsx';
import { ContainmentView } from './views/ContainmentView.tsx';
import { RetainersView } from './views/RetainersView.tsx';
import { DominatorsView } from './views/DominatorsView.tsx';
import { ContextsView } from './views/ContextsView.tsx';
import { HistoryView } from './views/HistoryView.tsx';
import { TimelineView } from './views/TimelineView.tsx';
import { DiffView } from './views/DiffView.tsx';

const TABS = [
  'Summary',
  'Containment',
  'Dominators',
  'Retainers',
  'Diff',
  'Contexts',
  'History',
  'Statistics',
  'Timeline',
] as const;
type Tab = (typeof TABS)[number];

type SnapshotInstance = ReturnType<typeof createSnapshot> & {
  tab: ReturnType<typeof createSignal<Tab>>;
  retainersNodeId: ReturnType<typeof createSignal<number | null>>;
  dominatorsNodeId: ReturnType<typeof createSignal<number | null>>;
  summaryHighlight: ReturnType<typeof createSignal<number | null>>;
  history: ReturnType<typeof createSignal<NodeInfo[]>>;
  reachableSizes: ReturnType<
    typeof createSignal<Map<number, ReachableSizeInfo>>
  >;
  reachablePending: ReturnType<typeof createSignal<Set<number>>>;
};

function createSnapshotInstance(): SnapshotInstance {
  return {
    ...createSnapshot(),
    tab: createSignal<Tab>('Summary'),
    retainersNodeId: createSignal<number | null>(null),
    dominatorsNodeId: createSignal<number | null>(null),
    summaryHighlight: createSignal<number | null>(null),
    history: createSignal<NodeInfo[]>([]),
    reachableSizes: createSignal<Map<number, ReachableSizeInfo>>(new Map()),
    reachablePending: createSignal<Set<number>>(new Set()),
  };
}

export function App(): JSX.Element {
  const [snapshots, setSnapshots] = createSignal<SnapshotInstance[]>([
    createSnapshotInstance(),
  ]);
  const [activeIndex, setActiveIndex] = createSignal(0);

  const active = () => snapshots()[activeIndex()];
  const [expandGcTarget, setExpandGcTarget] = createSignal<number | null>(null);

  const [menu, setMenu] = createSignal<{
    x: number;
    y: number;
    nodeId: number;
    edgeInfo?: EdgeInfo;
  } | null>(null);
  const [inspectInfo, setInspectInfo] = createSignal<{
    node: NodeInfo;
    edge?: EdgeInfo;
  } | null>(null);

  const historyStorageKey = (inst: SnapshotInstance): string | null => {
    const name = inst.filename();
    const hash = inst.contentHash();
    if (!name || !hash) return null;
    return `heap-history:${name}:${hash.slice(0, 16)}`;
  };

  const saveHistory = (inst: SnapshotInstance) => {
    const key = historyStorageKey(inst);
    if (!key) return;
    const ids = inst.history[0]().map((n) => n.id);
    try {
      localStorage.setItem(key, JSON.stringify(ids));
    } catch {
      // localStorage full or unavailable — ignore
    }
  };

  const restoreHistory = async (inst: SnapshotInstance) => {
    const key = historyStorageKey(inst);
    if (!key) return;
    try {
      const raw = localStorage.getItem(key);
      if (!raw) return;
      const ids: number[] = JSON.parse(raw);
      const infos = await Promise.all(
        ids.map((id) =>
          inst.call<NodeInfo>({ type: 'getNodeInfo', nodeId: id }),
        ),
      );
      const [, setHist] = inst.history;
      setHist(infos);
    } catch {
      // corrupt data or node IDs no longer valid — ignore
    }
  };

  const pushHistory = async (inst: SnapshotInstance, nodeId: number) => {
    const info = await inst.call<NodeInfo>({
      type: 'getNodeInfo',
      nodeId,
    });
    const [, setHist] = inst.history;
    setHist((prev) => {
      if (prev.length > 0 && prev[prev.length - 1].id === nodeId) return prev;
      return [...prev, info];
    });
    saveHistory(inst);
  };

  const navigate = (opts: NavigateOptions) => {
    const inst = active();
    const [, setTab] = inst.tab;
    const [, setRetainersNodeId] = inst.retainersNodeId;
    const [, setDominatorsNodeId] = inst.dominatorsNodeId;
    const [, setSummaryHighlight] = inst.summaryHighlight;
    if (opts.target === 'Retainers') {
      setRetainersNodeId(opts.nodeId);
      setTab('Retainers');
      pushHistory(inst, opts.nodeId);
    } else if (opts.target === 'Dominators') {
      setDominatorsNodeId(opts.nodeId);
      setTab('Dominators');
    } else {
      setSummaryHighlight(opts.nodeId);
      setTab('Summary');
    }
  };

  const handleContextMenu = (e: MouseEvent, nodeId: number, edgeInfo?: EdgeInfo) => {
    setMenu({ x: e.clientX, y: e.clientY, nodeId, edgeInfo });
  };

  onMount(() => {
    const handler = (e: KeyboardEvent) => {
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement
      )
        return;
      const idx = parseInt(e.key, 10) - 1;
      if (idx >= 0 && idx < TABS.length) {
        const [, setTab] = active().tab;
        setTab(TABS[idx]);
      }
    };
    document.addEventListener('keydown', handler);
    onCleanup(() => document.removeEventListener('keydown', handler));
  });

  const markPending = (inst: SnapshotInstance, nodeId: number) => {
    const [, setPending] = inst.reachablePending;
    setPending((prev) => {
      const next = new Set(prev);
      next.add(nodeId);
      return next;
    });
  };

  const storeReachable = (
    inst: SnapshotInstance,
    nodeId: number,
    info: ReachableSizeInfo,
  ) => {
    const [, setSizes] = inst.reachableSizes;
    const [, setPending] = inst.reachablePending;
    setSizes((prev) => {
      const next = new Map(prev);
      next.set(nodeId, info);
      return next;
    });
    setPending((prev) => {
      const next = new Set(prev);
      next.delete(nodeId);
      return next;
    });
  };

  const computeReachableSize = async (nodeId: number) => {
    const inst = active();
    markPending(inst, nodeId);
    const info = await inst.call<ReachableSizeInfo>({
      type: 'getReachableSize',
      nodeId,
    });
    storeReachable(inst, nodeId, info);
  };

  const computeReachableSizeWithChildren = async (nodeId: number) => {
    const inst = active();
    markPending(inst, nodeId);
    const info = await inst.call<ReachableSizeInfo>({
      type: 'getReachableSize',
      nodeId,
    });
    storeReachable(inst, nodeId, info);
    const childIds = await inst.call<number[]>({
      type: 'getChildrenIds',
      nodeId,
    });
    for (const id of childIds) markPending(inst, id);
    await Promise.all(
      childIds.map(async (id) => {
        const s = await inst.call<ReachableSizeInfo>({
          type: 'getReachableSize',
          nodeId: id,
        });
        storeReachable(inst, id, s);
      }),
    );
  };

  const handleLoadFile = async (file: File) => {
    const inst = active();
    if (inst.loaded()) {
      // Already loaded — create a new snapshot instance, stay on current
      const newInst = createSnapshotInstance();
      setSnapshots((prev) => [...prev, newInst]);
      await newInst.loadFile(file);
      restoreHistory(newInst);
    } else {
      await inst.loadFile(file);
      restoreHistory(inst);
    }
  };

  const closeSnapshot = async (index: number) => {
    const inst = snapshots()[index];
    await inst.close();
    const remaining = snapshots().filter((_, i) => i !== index);
    if (remaining.length === 0) {
      // Reset to a fresh empty instance
      setSnapshots([createSnapshotInstance()]);
      setActiveIndex(0);
    } else {
      setSnapshots(remaining);
      setActiveIndex(Math.min(activeIndex(), remaining.length - 1));
    }
  };

  const anyLoaded = () => snapshots().some((s) => s.loaded());

  return (
    <Show
      when={anyLoaded()}
      fallback={
        <div
          style={{ padding: '40px', 'font-family': 'system-ui, sans-serif' }}
        >
          <h1>Heap Snapshot Viewer</h1>
          <FileLoader
            loading={active().loading()}
            error={active().error()}
            onFile={handleLoadFile}
          />
        </div>
      }
    >
      <div class="app-shell">
        <div
          style={{
            display: 'flex',
            'align-items': 'center',
            gap: '8px',
            'margin-bottom': '8px',
          }}
        >
          <For each={snapshots()}>
            {(inst, i) => (
              <Show when={inst.loaded() || inst.loading()}>
                <span
                  style={{
                    display: 'inline-flex',
                    'align-items': 'center',
                    border:
                      i() === activeIndex() && inst.loaded()
                        ? '2px solid #333'
                        : '1px solid #ccc',
                    'border-radius': '4px',
                    background:
                      i() === activeIndex() && inst.loaded()
                        ? '#f0f0f0'
                        : 'white',
                    'font-size': '13px',
                  }}
                >
                  <button
                    onClick={() => {
                      if (inst.loaded()) setActiveIndex(i());
                    }}
                    disabled={!inst.loaded()}
                    style={{
                      padding: '4px 8px 4px 12px',
                      border: 'none',
                      background: 'none',
                      cursor: inst.loaded() ? 'pointer' : 'wait',
                      'font-weight':
                        i() === activeIndex() && inst.loaded()
                          ? 'bold'
                          : 'normal',
                      'font-size': '13px',
                    }}
                  >
                    {inst.filename() ?? `Snapshot ${i() + 1}`}
                    {inst.loading() && (
                      <span
                        style={{
                          display: 'inline-block',
                          width: '12px',
                          height: '12px',
                          'margin-left': '6px',
                          border: '2px solid #ccc',
                          'border-top-color': '#333',
                          'border-radius': '50%',
                          animation: 'spin 0.8s linear infinite',
                          'vertical-align': 'middle',
                        }}
                      />
                    )}
                  </button>
                  <Show when={inst.loaded()}>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        closeSnapshot(i());
                      }}
                      title="Close snapshot"
                      style={{
                        padding: '4px 8px 4px 4px',
                        border: 'none',
                        background: 'none',
                        cursor: 'pointer',
                        'font-size': '11px',
                        color: '#888',
                      }}
                    >
                      {'\u2715'}
                    </button>
                  </Show>
                </span>
              </Show>
            )}
          </For>
          <button
            onClick={() => {
              const input = document.createElement('input');
              input.type = 'file';
              input.accept = '.heapsnapshot,.heaptimeline';
              input.onchange = () => {
                const file = input.files?.[0];
                if (file) handleLoadFile(file);
              };
              input.click();
            }}
            style={{
              padding: '4px 12px',
              border: '1px solid #ccc',
              'border-radius': '4px',
              background: 'white',
              cursor: 'pointer',
              'font-size': '13px',
              'margin-left': 'auto',
            }}
          >
            + Load snapshot
          </button>
        </div>

        <TabNav
          tabs={TABS}
          active={active().tab[0]()}
          onChange={(t) => active().tab[1](t)}
          disabled={(() => {
            const disabled = new Set<Tab>();
            if (!active().hasAllocationData()) disabled.add('Timeline');
            if (snapshots().filter((s) => s.loaded()).length < 2)
              disabled.add('Diff');
            return disabled.size > 0 ? disabled : undefined;
          })()}
        />

        <For each={snapshots()}>
          {(inst, i) => (
            <div
              data-testid={`snapshot-${i()}`}
              style={{
                display: i() === activeIndex() ? 'flex' : 'none',
                'flex-direction': 'column',
                'min-height': '0',
                flex: '1',
              }}
            >
              <Show
                when={inst.loaded()}
                fallback={
                  <Show when={inst.loading()}>
                    <p style={{ 'margin-top': '16px', color: '#888' }}>
                      Loading snapshot...
                    </p>
                  </Show>
                }
              >
                <div class="tab-panel" hidden={inst.tab[0]() !== 'Summary'}>
                  <SummaryView
                    call={inst.call}
                    onNavigate={navigate}
                    onContextMenu={handleContextMenu}
                    highlightNodeId={inst.summaryHighlight[0]()}
                    reachableSizes={inst.reachableSizes[0]()}
                    reachablePending={inst.reachablePending[0]()}
                  />
                </div>
                <div class="tab-panel" hidden={inst.tab[0]() !== 'Containment'}>
                  <ContainmentView
                    call={inst.call}
                    onNavigate={navigate}
                    onContextMenu={handleContextMenu}
                    reachableSizes={inst.reachableSizes[0]()}
                    reachablePending={inst.reachablePending[0]()}
                  />
                </div>
                <div class="tab-panel" hidden={inst.tab[0]() !== 'Dominators'}>
                  <DominatorsView
                    call={inst.call}
                    onNavigate={navigate}
                    onContextMenu={handleContextMenu}
                    focusNodeId={inst.dominatorsNodeId[0]()}
                    reachableSizes={inst.reachableSizes[0]()}
                    reachablePending={inst.reachablePending[0]()}
                  />
                </div>
                <div class="tab-panel" hidden={inst.tab[0]() !== 'Retainers'}>
                  <RetainersView
                    call={inst.call}
                    nodeId={inst.retainersNodeId[0]()}
                    onNavigate={navigate}
                    onContextMenu={handleContextMenu}
                    expandGcTarget={expandGcTarget}
                    reachableSizes={inst.reachableSizes[0]()}
                    reachablePending={inst.reachablePending[0]()}
                  />
                </div>
                <div class="tab-panel" hidden={inst.tab[0]() !== 'Diff'}>
                  <DiffView
                    call={inst.call}
                    snapshotId={inst.snapshotId}
                    snapshots={snapshots}
                    currentIndex={i}
                  />
                </div>
                <div class="tab-panel" hidden={inst.tab[0]() !== 'Contexts'}>
                  <ContextsView
                    call={inst.call}
                    onNavigate={navigate}
                    onContextMenu={handleContextMenu}
                    reachableSizes={inst.reachableSizes[0]()}
                    reachablePending={inst.reachablePending[0]()}
                    onReachableSize={(nodeId, info) =>
                      storeReachable(inst, nodeId, info)
                    }
                    onMarkPending={(nodeId) => markPending(inst, nodeId)}
                  />
                </div>
                <div class="tab-panel" hidden={inst.tab[0]() !== 'History'}>
                  <HistoryView
                    call={inst.call}
                    history={inst.history[0]()}
                    onNavigate={navigate}
                    onContextMenu={handleContextMenu}
                    reachableSizes={inst.reachableSizes[0]()}
                    reachablePending={inst.reachablePending[0]()}
                  />
                </div>
                <div class="tab-panel" hidden={inst.tab[0]() !== 'Statistics'}>
                  <StatisticsView call={inst.call} />
                </div>
                <div class="tab-panel" hidden={inst.tab[0]() !== 'Timeline'}>
                  <TimelineView
                    call={inst.call}
                    onNavigate={navigate}
                    onContextMenu={handleContextMenu}
                    reachableSizes={inst.reachableSizes[0]()}
                    reachablePending={inst.reachablePending[0]()}
                  />
                </div>
              </Show>
            </div>
          )}
        </For>

        <Show when={menu()}>
          {(m) => (
            <ContextMenu
              x={m().x}
              y={m().y}
              onClose={() => setMenu(null)}
              items={(() => {
                const nodeId = m().nodeId;
                const edge = m().edgeInfo;
                return [
                {
                  label: 'Show retainers',
                  action: () =>
                    navigate({ nodeId, target: 'Retainers' }),
                },
                {
                  label: 'Show in dominators',
                  action: () =>
                    navigate({ nodeId, target: 'Dominators' }),
                },
                {
                  label: 'Show in summary',
                  action: () =>
                    navigate({ nodeId, target: 'Summary' }),
                },
                {
                  label: 'Expand path to GC roots',
                  action: () => {
                    const inst = active();
                    const [, setTab] = inst.tab;
                    const [, setRetainersNodeId] = inst.retainersNodeId;
                    if (inst.tab[0]() !== 'Retainers') {
                      setRetainersNodeId(nodeId);
                      setTab('Retainers');
                      pushHistory(inst, nodeId);
                    }
                    setExpandGcTarget(null);
                    queueMicrotask(() => setExpandGcTarget(nodeId));
                  },
                },
                {
                  label: 'Remember object',
                  action: () => pushHistory(active(), nodeId),
                },
                {
                  label: 'Compute reachable size',
                  action: () => computeReachableSize(nodeId),
                },
                {
                  label: 'Compute reachable size w/ children',
                  action: () => computeReachableSizeWithChildren(nodeId),
                },
                {
                  label: 'Inspect',
                  action: async () => {
                    const info = await active().call<NodeInfo>({
                      type: 'getNodeInfo',
                      nodeId,
                    });
                    setInspectInfo({ node: info, edge });
                  },
                },
              ];
              })()}
            />
          )}
        </Show>
        <Show when={inspectInfo()}>
          {(data) => (
            <InspectDialog
              info={data().node}
              edgeInfo={data().edge}
              onClose={() => setInspectInfo(null)}
            />
          )}
        </Show>
      </div>
    </Show>
  );
}
