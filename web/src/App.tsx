import { createSignal, Show, For, onCleanup, onMount, type JSX } from 'solid-js';
import { createSnapshot } from './worker/use-snapshot.ts';
import { FileLoader } from './components/FileLoader.tsx';
import { TabNav } from './components/TabNav.tsx';
import { ContextMenu } from './components/ContextMenu.tsx';
import type { NavigateOptions } from './components/ObjectLink.tsx';
import type { ReachableSizeInfo, NodeInfo } from './types.ts';
import { StatisticsView } from './views/StatisticsView.tsx';
import { SummaryView } from './views/SummaryView.tsx';
import { ContainmentView } from './views/ContainmentView.tsx';
import { RetainersView } from './views/RetainersView.tsx';
import { DominatorsView } from './views/DominatorsView.tsx';
import { ContextsView } from './views/ContextsView.tsx';
import { HistoryView } from './views/HistoryView.tsx';

const TABS = [
  'Summary',
  'Containment',
  'Dominators',
  'Retainers',
  'Contexts',
  'History',
  'Statistics',
] as const;
type Tab = (typeof TABS)[number];

type SnapshotInstance = ReturnType<typeof createSnapshot> & {
  tab: ReturnType<typeof createSignal<Tab>>;
  retainersNodeId: ReturnType<typeof createSignal<number | null>>;
  dominatorsNodeId: ReturnType<typeof createSignal<number | null>>;
  summaryHighlight: ReturnType<typeof createSignal<number | null>>;
  history: ReturnType<typeof createSignal<NodeInfo[]>>;
};

function createSnapshotInstance(): SnapshotInstance {
  return {
    ...createSnapshot(),
    tab: createSignal<Tab>('Summary'),
    retainersNodeId: createSignal<number | null>(null),
    dominatorsNodeId: createSignal<number | null>(null),
    summaryHighlight: createSignal<number | null>(null),
    history: createSignal<NodeInfo[]>([]),
  };
}

export function App(): JSX.Element {
  const [snapshots, setSnapshots] = createSignal<SnapshotInstance[]>([
    createSnapshotInstance(),
  ]);
  const [activeIndex, setActiveIndex] = createSignal(0);

  const active = () => snapshots()[activeIndex()];

  const [menu, setMenu] = createSignal<{
    x: number;
    y: number;
    nodeId: number;
  } | null>(null);

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

  const handleContextMenu = (e: MouseEvent, nodeId: number) => {
    setMenu({ x: e.clientX, y: e.clientY, nodeId });
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

  const computeReachableSize = async (nodeId: number) => {
    const info = await active().call<ReachableSizeInfo>({
      type: 'getReachableSize',
      nodeId,
    });
    console.log('Reachable size for', nodeId, info);
  };

  const computeReachableSizeWithChildren = async (nodeId: number) => {
    const inst = active();
    const info = await inst.call<ReachableSizeInfo>({
      type: 'getReachableSize',
      nodeId,
    });
    console.log('Reachable size for', nodeId, info);
    const childIds = await inst.call<number[]>({
      type: 'getChildrenIds',
      nodeId,
    });
    const childInfos = await Promise.all(
      childIds.map((id) =>
        inst
          .call<ReachableSizeInfo>({ type: 'getReachableSize', nodeId: id })
          .then((s) => [id, s] as const),
      ),
    );
    for (const [id, s] of childInfos) {
      console.log('  child', id, s);
    }
  };

  const handleLoadFile = (file: File) => {
    const inst = active();
    if (inst.loaded()) {
      // Already loaded — create a new snapshot instance
      const newInst = createSnapshotInstance();
      const newIndex = snapshots().length;
      setSnapshots((prev) => [...prev, newInst]);
      setActiveIndex(newIndex);
      newInst.loadFile(file);
    } else {
      inst.loadFile(file);
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
      <div style={{ 'font-family': 'system-ui, sans-serif', padding: '16px' }}>
        <div
          style={{
            display: 'flex',
            'align-items': 'center',
            gap: '8px',
            'margin-bottom': '8px',
          }}
        >
          <Show when={snapshots().length > 1}>
            <For each={snapshots()}>
              {(inst, i) => (
                <Show when={inst.loaded()}>
                  <button
                    onClick={() => setActiveIndex(i())}
                    style={{
                      padding: '4px 12px',
                      border:
                        i() === activeIndex()
                          ? '2px solid #333'
                          : '1px solid #ccc',
                      'border-radius': '4px',
                      background: i() === activeIndex() ? '#f0f0f0' : 'white',
                      cursor: 'pointer',
                      'font-size': '13px',
                      'font-weight': i() === activeIndex() ? 'bold' : 'normal',
                    }}
                  >
                    {inst.filename() ?? `Snapshot ${i() + 1}`}
                  </button>
                </Show>
              )}
            </For>
          </Show>
          <Show when={snapshots().length <= 1}>
            <span style={{ 'font-weight': 'bold', 'font-size': '14px' }}>
              {active().filename()}
            </span>
          </Show>
          <button
            onClick={() => {
              const input = document.createElement('input');
              input.type = 'file';
              input.accept = '.heapsnapshot';
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
        />

        <For each={snapshots()}>
          {(inst, i) => (
            <div
              style={{
                display: i() === activeIndex() ? undefined : 'none',
              }}
            >
              <Show when={inst.loaded()} fallback={
                <Show when={inst.loading()}>
                  <p style={{ 'margin-top': '16px', color: '#888' }}>Loading snapshot...</p>
                </Show>
              }>
              <div
                style={{
                  'margin-top': '16px',
                  display: inst.tab[0]() === 'Summary' ? undefined : 'none',
                }}
              >
                <SummaryView
                  call={inst.call}
                  onNavigate={navigate}
                  onContextMenu={handleContextMenu}
                  highlightNodeId={inst.summaryHighlight[0]()}
                />
              </div>
              <div
                style={{
                  'margin-top': '16px',
                  display:
                    inst.tab[0]() === 'Containment' ? undefined : 'none',
                }}
              >
                <ContainmentView
                  call={inst.call}
                  onNavigate={navigate}
                  onContextMenu={handleContextMenu}
                />
              </div>
              <div
                style={{
                  'margin-top': '16px',
                  display: inst.tab[0]() === 'Dominators' ? undefined : 'none',
                }}
              >
                <DominatorsView
                  call={inst.call}
                  onNavigate={navigate}
                  onContextMenu={handleContextMenu}
                  focusNodeId={inst.dominatorsNodeId[0]()}
                />
              </div>
              <div
                style={{
                  'margin-top': '16px',
                  display: inst.tab[0]() === 'Retainers' ? undefined : 'none',
                }}
              >
                <RetainersView
                  call={inst.call}
                  nodeId={inst.retainersNodeId[0]()}
                  onNavigate={navigate}
                  onContextMenu={handleContextMenu}
                />
              </div>
              <div
                style={{
                  'margin-top': '16px',
                  display: inst.tab[0]() === 'Contexts' ? undefined : 'none',
                }}
              >
                <ContextsView
                  call={inst.call}
                  onNavigate={navigate}
                  onContextMenu={handleContextMenu}
                />
              </div>
              <div
                style={{
                  'margin-top': '16px',
                  display: inst.tab[0]() === 'History' ? undefined : 'none',
                }}
              >
                <HistoryView
                  call={inst.call}
                  history={inst.history[0]()}
                  onNavigate={navigate}
                  onContextMenu={handleContextMenu}
                />
              </div>
              <div
                style={{
                  'margin-top': '16px',
                  display: inst.tab[0]() === 'Statistics' ? undefined : 'none',
                }}
              >
                <StatisticsView call={inst.call} />
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
              items={[
                {
                  label: 'Show retainers',
                  action: () =>
                    navigate({ nodeId: m().nodeId, target: 'Retainers' }),
                },
                {
                  label: 'Show in dominators',
                  action: () =>
                    navigate({ nodeId: m().nodeId, target: 'Dominators' }),
                },
                {
                  label: 'Show in summary',
                  action: () =>
                    navigate({ nodeId: m().nodeId, target: 'Summary' }),
                },
                {
                  label: 'Compute reachable size',
                  action: () => computeReachableSize(m().nodeId),
                },
                {
                  label: 'Compute reachable size w/ children',
                  action: () => computeReachableSizeWithChildren(m().nodeId),
                },
              ]}
            />
          )}
        </Show>
      </div>
    </Show>
  );
}
