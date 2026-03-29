import { createSignal, Show, onCleanup, onMount, type JSX } from 'solid-js';
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

export function App(): JSX.Element {
  const snapshot = createSnapshot();
  const [tab, setTab] = createSignal<Tab>('Summary');
  const [retainersNodeId, setRetainersNodeId] = createSignal<number | null>(
    null,
  );
  const [dominatorsNodeId, setDominatorsNodeId] = createSignal<number | null>(
    null,
  );
  const [summaryHighlight, setSummaryHighlight] = createSignal<number | null>(
    null,
  );
  const [menu, setMenu] = createSignal<{
    x: number;
    y: number;
    nodeId: number;
  } | null>(null);
  const [history, setHistory] = createSignal<NodeInfo[]>([]);

  const pushHistory = async (nodeId: number) => {
    const info = await snapshot.call<NodeInfo>({
      type: 'getNodeInfo',
      nodeId,
    });
    setHistory((prev) => {
      if (prev.length > 0 && prev[prev.length - 1].id === nodeId) return prev;
      return [...prev, info];
    });
  };

  const navigate = (opts: NavigateOptions) => {
    if (opts.target === 'Retainers') {
      setRetainersNodeId(opts.nodeId);
      setTab('Retainers');
      pushHistory(opts.nodeId);
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
        setTab(TABS[idx]);
      }
    };
    document.addEventListener('keydown', handler);
    onCleanup(() => document.removeEventListener('keydown', handler));
  });

  const computeReachableSize = async (nodeId: number) => {
    // TODO: integrate with reachable sizes store
    const info = await snapshot.call<ReachableSizeInfo>({
      type: 'getReachableSize',
      nodeId,
    });
    console.log('Reachable size for', nodeId, info);
  };

  const computeReachableSizeWithChildren = async (nodeId: number) => {
    const info = await snapshot.call<ReachableSizeInfo>({
      type: 'getReachableSize',
      nodeId,
    });
    console.log('Reachable size for', nodeId, info);
    const childIds = await snapshot.call<number[]>({
      type: 'getChildrenIds',
      nodeId,
    });
    const childInfos = await Promise.all(
      childIds.map((id) =>
        snapshot
          .call<ReachableSizeInfo>({ type: 'getReachableSize', nodeId: id })
          .then((s) => [id, s] as const),
      ),
    );
    for (const [id, s] of childInfos) {
      console.log('  child', id, s);
    }
  };

  return (
    <Show
      when={snapshot.loaded()}
      fallback={
        <div
          style={{ padding: '40px', 'font-family': 'system-ui, sans-serif' }}
        >
          <h1>Heap Snapshot Viewer</h1>
          <FileLoader
            loading={snapshot.loading()}
            error={snapshot.error()}
            onFile={snapshot.loadFile}
          />
        </div>
      }
    >
      <div style={{ 'font-family': 'system-ui, sans-serif', padding: '16px' }}>
        <TabNav tabs={TABS} active={tab()} onChange={setTab} />

        <div
          style={{
            'margin-top': '16px',
            display: tab() === 'Summary' ? undefined : 'none',
          }}
        >
          <SummaryView
            call={snapshot.call}
            onNavigate={navigate}
            onContextMenu={handleContextMenu}
            highlightNodeId={summaryHighlight()}
          />
        </div>
        <div
          style={{
            'margin-top': '16px',
            display: tab() === 'Containment' ? undefined : 'none',
          }}
        >
          <ContainmentView
            call={snapshot.call}
            onNavigate={navigate}
            onContextMenu={handleContextMenu}
          />
        </div>
        <div
          style={{
            'margin-top': '16px',
            display: tab() === 'Dominators' ? undefined : 'none',
          }}
        >
          <DominatorsView
            call={snapshot.call}
            onNavigate={navigate}
            onContextMenu={handleContextMenu}
            focusNodeId={dominatorsNodeId()}
          />
        </div>
        <div
          style={{
            'margin-top': '16px',
            display: tab() === 'Retainers' ? undefined : 'none',
          }}
        >
          <RetainersView
            call={snapshot.call}
            nodeId={retainersNodeId()}
            onNavigate={navigate}
            onContextMenu={handleContextMenu}
          />
        </div>
        <div
          style={{
            'margin-top': '16px',
            display: tab() === 'Contexts' ? undefined : 'none',
          }}
        >
          <ContextsView
            call={snapshot.call}
            onNavigate={navigate}
            onContextMenu={handleContextMenu}
          />
        </div>
        <div
          style={{
            'margin-top': '16px',
            display: tab() === 'History' ? undefined : 'none',
          }}
        >
          <HistoryView
            call={snapshot.call}
            history={history()}
            onNavigate={navigate}
            onContextMenu={handleContextMenu}
          />
        </div>
        <div
          style={{
            'margin-top': '16px',
            display: tab() === 'Statistics' ? undefined : 'none',
          }}
        >
          <StatisticsView call={snapshot.call} />
        </div>

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
