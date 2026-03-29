import { createContext, useContext, useState, type ReactNode } from 'react';
import type { RowSelection } from './TreeTable.tsx';
import type { ReachableSizeInfo } from '../types.ts';

// Per-view selection context
interface SelectionState {
  selection: RowSelection | null;
  onSelect: (sel: RowSelection) => void;
}

const SelectionContext = createContext<SelectionState>({
  selection: null,
  onSelect: () => {},
});

export function SelectionProvider({ children }: { children: ReactNode }) {
  const [selection, setSelection] = useState<RowSelection | null>(null);
  return (
    <SelectionContext.Provider value={{ selection, onSelect: setSelection }}>
      {children}
    </SelectionContext.Provider>
  );
}

export function useSelection() {
  return useContext(SelectionContext);
}

// Global reachable sizes shared across all views
const ReachableSizesContext = createContext<Map<number, ReachableSizeInfo>>(
  new Map(),
);

export const ReachableSizesProvider = ReachableSizesContext.Provider;

export function useReachableSizes() {
  return useContext(ReachableSizesContext);
}
