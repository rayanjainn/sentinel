// Process hierarchy from ppid, flattened into rows for a virtualized tree.
import type { ProcessInfo } from "../../bindings/ProcessInfo";

/** PIDs are recycled; expansion state keys on pid + start time so a new process starts fresh. */
export const processKey = (p: Pick<ProcessInfo, "pid" | "startTime">) => `${p.pid}:${p.startTime}`;

export interface ProcessForest {
  roots: number[];
  children: Map<number, number[]>;
  byPid: Map<number, ProcessInfo>;
}

export function buildForest(processes: ProcessInfo[]): ProcessForest {
  const byPid = new Map<number, ProcessInfo>();
  for (const p of processes) byPid.set(p.pid, p);
  const children = new Map<number, number[]>();
  const roots: number[] = [];
  for (const p of processes) {
    const parent = p.ppid;
    if (parent === null || parent === p.pid || !byPid.has(parent)) {
      roots.push(p.pid);
      continue;
    }
    const list = children.get(parent);
    if (list) list.push(p.pid);
    else children.set(parent, [p.pid]);
  }
  // Cycles (a→b→a with no root) would otherwise vanish entirely; promote one member of each.
  const reachable = new Set<number>();
  const stack = [...roots];
  while (stack.length) {
    const pid = stack.pop()!;
    if (reachable.has(pid)) continue;
    reachable.add(pid);
    for (const c of children.get(pid) ?? []) stack.push(c);
  }
  if (reachable.size < processes.length) {
    for (const p of processes) {
      if (reachable.has(p.pid)) continue;
      roots.push(p.pid);
      const queue = [p.pid];
      while (queue.length) {
        const pid = queue.pop()!;
        if (reachable.has(pid)) continue;
        reachable.add(pid);
        for (const c of children.get(pid) ?? []) if (c !== p.pid) queue.push(c);
      }
      // Detach the promoted root from its cyclic parent.
      if (p.ppid !== null) {
        const siblings = children.get(p.ppid);
        if (siblings) children.set(p.ppid, siblings.filter((c) => c !== p.pid));
      }
    }
  }
  return { roots, children, byPid };
}

export interface TreeRow {
  process: ProcessInfo;
  key: string;
  depth: number;
  hasChildren: boolean;
  expanded: boolean;
  /** Descendants currently hidden or shown under this node (for collapsed summaries). */
  descendantCount: number;
  /** For each ancestor level: whether a vertical guide continues past this row. */
  guides: boolean[];
  isLast: boolean;
}

export interface FlattenOptions {
  collapsed: ReadonlySet<string>;
  compare: (a: ProcessInfo, b: ProcessInfo) => number;
  /** When set, only matches and their ancestors are shown, with ancestors forced open. */
  match?: ((p: ProcessInfo) => boolean) | null;
}

export function flattenForest(forest: ProcessForest, options: FlattenOptions): TreeRow[] {
  const { children, byPid } = forest;
  const { collapsed, compare, match } = options;

  const descendants = new Map<number, number>();
  const countDescendants = (pid: number, seen: Set<number>): number => {
    const cached = descendants.get(pid);
    if (cached !== undefined) return cached;
    if (seen.has(pid)) return 0;
    seen.add(pid);
    let total = 0;
    for (const c of children.get(pid) ?? []) total += 1 + countDescendants(c, seen);
    descendants.set(pid, total);
    return total;
  };

  let visible: Set<number> | null = null;
  if (match) {
    visible = new Set();
    const markMatches = (pid: number, seen: Set<number>): boolean => {
      if (seen.has(pid)) return false;
      seen.add(pid);
      const p = byPid.get(pid)!;
      let any = match(p);
      for (const c of children.get(pid) ?? []) if (markMatches(c, seen)) any = true;
      if (any) visible!.add(pid);
      return any;
    };
    const seen = new Set<number>();
    for (const r of forest.roots) markMatches(r, seen);
  }

  const rows: TreeRow[] = [];
  const emitted = new Set<number>();
  const sortPids = (pids: number[]) =>
    pids
      .filter((pid) => (visible ? visible.has(pid) : true))
      .map((pid) => byPid.get(pid)!)
      .sort(compare);

  const walk = (siblings: ProcessInfo[], depth: number, guides: boolean[]) => {
    siblings.forEach((p, i) => {
      if (emitted.has(p.pid)) return;
      emitted.add(p.pid);
      const kids = sortPids(children.get(p.pid) ?? []);
      const key = processKey(p);
      const isLast = i === siblings.length - 1;
      const expanded = kids.length > 0 && (visible !== null || !collapsed.has(key));
      rows.push({
        process: p,
        key,
        depth,
        hasChildren: kids.length > 0,
        expanded,
        descendantCount: countDescendants(p.pid, new Set()),
        guides,
        isLast,
      });
      if (expanded) walk(kids, depth + 1, [...guides, !isLast]);
    });
  };
  walk(sortPids(forest.roots), 0, []);
  return rows;
}

/** Keys of every node with children, for "collapse all". */
export function parentKeys(forest: ProcessForest): string[] {
  const keys: string[] = [];
  for (const [pid, kids] of forest.children) {
    const p = forest.byPid.get(pid);
    if (p && kids.length > 0) keys.push(processKey(p));
  }
  return keys;
}
