import { useState, useCallback, useEffect } from 'react';
import { useSnapshot } from './worker/use-snapshot.ts';
import { FileLoader } from './components/FileLoader.tsx';
import { TabNav } from './components/TabNav.tsx';
import { ContextMenu } from './components/ContextMenu.tsx';
import type { NavigateOptions } from './components/ObjectLink.tsx';
import {
  SelectionProvider,
  ReachableSizesProvider,
} from './components/SelectionContext.tsx';
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

export function App() {
  const snapshot = useSnapshot();
  const [tab, setTab] = useState<Tab>('Summary');
  const [retainersNodeId, setRetainersNodeId] = useState<number | null>(null);
  const [dominatorsNodeId, setDominatorsNodeId] = useState<number | null>(null);
  const [summaryHighlight, setSummaryHighlight] = useState<number | null>(null);
  const [menu, setMenu] = useState<{
    x: number;
    y: number;
    nodeId: number;
  } | null>(null);
  const [reachableSizes, setReachableSizes] = useState<
    Map<number, ReachableSizeInfo>
  >(new Map());
  const [history, setHistory] = useState<NodeInfo[]>([]);

  const pushHistory = useCallback(
    async (nodeId: number) => {
      const info = await snapshot.call<NodeInfo>({
        type: 'getNodeInfo',
        nodeId,
      });
      setHistory((prev) => {
        if (prev.length > 0 && prev[prev.length - 1].id === nodeId) return prev;
        return [...prev, info];
      });
    },
    [snapshot.call],
  );

  const navigate = useCallback(
    (opts: NavigateOptions) => {
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
    },
    [pushHistory],
  );

  const handleContextMenu = useCallback(
    (e: React.MouseEvent, nodeId: number) => {
      setMenu({ x: e.clientX, y: e.clientY, nodeId });
    },
    [],
  );

  useEffect(() => {
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
    return () => document.removeEventListener('keydown', handler);
  }, []);

  const computeReachableSize = useCallback(
    async (nodeId: number) => {
      const info = await snapshot.call<ReachableSizeInfo>({
        type: 'getReachableSize',
        nodeId,
      });
      setReachableSizes((prev) => new Map(prev).set(nodeId, info));
    },
    [snapshot.call],
  );

  const computeReachableSizeWithChildren = useCallback(
    async (nodeId: number) => {
      const info = await snapshot.call<ReachableSizeInfo>({
        type: 'getReachableSize',
        nodeId,
      });
      const childIds = await snapshot.call<number[]>({
        type: 'getChildrenIds',
        nodeId,
      });
      const updates = new Map<number, ReachableSizeInfo>();
      updates.set(nodeId, info);
      const childInfos = await Promise.all(
        childIds.map((id) =>
          snapshot
            .call<ReachableSizeInfo>({ type: 'getReachableSize', nodeId: id })
            .then((s) => [id, s] as const),
        ),
      );
      for (const [id, s] of childInfos) {
        updates.set(id, s);
      }
      setReachableSizes((prev) => {
        const next = new Map(prev);
        for (const [id, s] of updates) {
          next.set(id, s);
        }
        return next;
      });
    },
    [snapshot.call],
  );

  if (!snapshot.loaded) {
    return (
      <div style={{ padding: 40, fontFamily: 'system-ui, sans-serif' }}>
        <h1>Heap Snapshot Viewer</h1>
        <FileLoader
          loading={snapshot.loading}
          error={snapshot.error}
          onFile={snapshot.loadFile}
        />
      </div>
    );
  }

  const viewProps = {
    call: snapshot.call,
    onNavigate: navigate,
    onContextMenu: handleContextMenu,
  };

  return (
    <ReachableSizesProvider value={reachableSizes}>
      <div style={{ fontFamily: 'system-ui, sans-serif', padding: 16 }}>
        <TabNav tabs={TABS} active={tab} onChange={setTab} />
        <div
          style={{
            marginTop: 16,
            display: tab === 'Summary' ? undefined : 'none',
          }}
        >
          <SummaryView {...viewProps} highlightNodeId={summaryHighlight} />
        </div>
        <div
          style={{
            marginTop: 16,
            display: tab === 'Containment' ? undefined : 'none',
          }}
        >
          <SelectionProvider>
            <ContainmentView {...viewProps} />
          </SelectionProvider>
        </div>
        <div
          style={{
            marginTop: 16,
            display: tab === 'Dominators' ? undefined : 'none',
          }}
        >
          <SelectionProvider>
            <DominatorsView {...viewProps} focusNodeId={dominatorsNodeId} />
          </SelectionProvider>
        </div>
        <div
          style={{
            marginTop: 16,
            display: tab === 'Retainers' ? undefined : 'none',
          }}
        >
          <SelectionProvider>
            <RetainersView {...viewProps} nodeId={retainersNodeId} />
          </SelectionProvider>
        </div>
        <div
          style={{
            marginTop: 16,
            display: tab === 'Contexts' ? undefined : 'none',
          }}
        >
          <SelectionProvider>
            <ContextsView {...viewProps} />
          </SelectionProvider>
        </div>
        <div
          style={{
            marginTop: 16,
            display: tab === 'History' ? undefined : 'none',
          }}
        >
          <SelectionProvider>
            <HistoryView {...viewProps} history={history} />
          </SelectionProvider>
        </div>
        <div
          style={{
            marginTop: 16,
            display: tab === 'Statistics' ? undefined : 'none',
          }}
        >
          <StatisticsView call={snapshot.call} />
        </div>

        {menu && (
          <ContextMenu
            x={menu.x}
            y={menu.y}
            onClose={() => setMenu(null)}
            items={[
              {
                label: 'Show retainers',
                action: () =>
                  navigate({ nodeId: menu.nodeId, target: 'Retainers' }),
              },
              {
                label: 'Show in dominators',
                action: () =>
                  navigate({ nodeId: menu.nodeId, target: 'Dominators' }),
              },
              {
                label: 'Show in summary',
                action: () =>
                  navigate({ nodeId: menu.nodeId, target: 'Summary' }),
              },
              {
                label: 'Compute reachable size',
                action: () => computeReachableSize(menu.nodeId),
              },
              {
                label: 'Compute reachable size w/ children',
                action: () => computeReachableSizeWithChildren(menu.nodeId),
              },
            ]}
          />
        )}
      </div>
    </ReachableSizesProvider>
  );
}
