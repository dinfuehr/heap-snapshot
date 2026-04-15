export interface Statistics {
  node_count: number;
  total: number;
  v8heap_total: number;
  native_total: number;
  code: number;
  strings: number;
  js_arrays: number;
  typed_arrays: number;
  system: number;
  extra_native_bytes: number;
  unreachable_size: number;
  unreachable_count: number;
  context_sizes: { label: string; size: number }[];
  shared_size: number;
  unattributed_size: number;
}

export interface AggregateEntry {
  index: number;
  name: string;
  count: number;
  self_size: number;
  retained_size: number;
}

export interface NodeInfo {
  id: number;
  name: string;
  node_type: string;
  self_size: number;
  retained_size: number;
  distance: number;
  edge_count: number;
  detachedness: number; // 0=unknown, 1=attached, 2=detached
  ctx: string;
  ctx_label: string;
}

export interface Edge {
  edge_type: string;
  edge_name: string;
  target: NodeInfo;
}

export interface Retainer {
  edge_type: string;
  edge_name: string;
  source: NodeInfo;
}

export interface EdgeWithChildCount extends Edge {
  child_count: number;
}

export interface Containment {
  system_roots: Edge[];
  gc_roots_children: EdgeWithChildCount[];
}

export interface Children {
  total: number;
  edges: Edge[];
}

export interface Retainers {
  total: number;
  retainers: Retainer[];
}

export interface SummaryObject {
  id: number;
  name: string;
  self_size: number;
  retained_size: number;
  detachedness: number;
  ctx: string;
  ctx_label: string;
}

export interface SummaryExpanded {
  constructor: string;
  total: number;
  objects: SummaryObject[];
}

export interface RetainingPath {
  edge_type: string;
  edge_name: string;
  node: NodeInfo;
  children: RetainingPath[];
}

export interface RetainingPaths {
  target: NodeInfo;
  paths: RetainingPath[];
  reached_gc_roots: boolean;
  truncated: boolean;
}

export interface NativeContext {
  id: number;
  label: string;
  detachedness: string;
  self_size: number;
  retained_size: number;
  vars: string;
}

export interface ReachableSizeInfo {
  size: number;
  native_contexts: NativeContext[];
}

export interface DominatedChildren {
  total: number;
  children: NodeInfo[];
}

export interface AllocationFrame {
  function_name: string;
  script_name: string;
  line: number;
  column: number;
}

export interface AllocationStack {
  frames: AllocationFrame[];
}

export interface TimelineInterval {
  timestamp_us: number;
  count: number;
  size: number;
}

export interface ClassDiff {
  name: string;
  new_count: number;
  deleted_count: number;
  delta_count: number;
  alloc_size: number;
  freed_size: number;
  size_delta: number;
}

export interface Dominator {
  id: number;
  name: string;
  node_type: string;
  self_size: number;
  retained_size: number;
}

export type WorkerRequest =
  | { id: number; type: 'load'; data: ArrayBuffer }
  | { id: number; type: 'close'; snapshotId: number }
  | { id: number; type: 'getStatistics' }
  | { id: number; type: 'getSummaryWithFilter'; mode: number }
  | {
      id: number;
      type: 'getSummaryWithContextFilter';
      contextMode: number;
      contextIndex: number;
    }
  | {
      id: number;
      type: 'getSummaryObjects';
      constructorIndex: number;
      offset: number;
      limit: number;
    }
  | { id: number; type: 'getContainment' }
  | {
      id: number;
      type: 'getChildren';
      nodeId: number;
      offset: number;
      limit: number;
    }
  | {
      id: number;
      type: 'getRetainers';
      nodeId: number;
      offset: number;
      limit: number;
    }
  | {
      id: number;
      type: 'getRetainingPaths';
      nodeId: number;
      maxDepth: number;
      maxNodes: number;
    }
  | { id: number; type: 'getNativeContexts' }
  | { id: number; type: 'getDominatorsOf'; nodeId: number }
  | { id: number; type: 'getNodeInfo'; nodeId: number }
  | { id: number; type: 'getAllocationStack'; nodeId: number }
  | { id: number; type: 'getSummaryForInterval'; intervalIndex: number }
  | {
      id: number;
      type: 'getTimelineObjects';
      constructorIndex: number;
      offset: number;
      limit: number;
    }
  | { id: number; type: 'getTimeline' };

export type WorkerResponse =
  | { id: number; type: 'success'; data: unknown }
  | { id: number; type: 'error'; error: string }
  | { type: 'progress'; message: string };
