// Groups helper processes under their owning app for the "Grouped" table layout: "Brave Browser —
// 41 processes, 3.2 GB" collapsible to its helpers. A recognised app with only one process (most
// non-browser apps) is shown as a normal row rather than a one-item group.
import type { ProcessInfo } from "../../bindings/ProcessInfo";

export interface AppGroup {
  key: string;
  appName: string;
  members: ProcessInfo[];
  totalMemory: number;
  totalCpu: number;
}

export type AppGroupItem = { kind: "group"; group: AppGroup } | { kind: "single"; process: ProcessInfo };

export function groupByApp(processes: ProcessInfo[]): AppGroupItem[] {
  const byApp = new Map<string, AppGroup>();
  for (const p of processes) {
    const app = p.summary.appName;
    if (!app) continue;
    let g = byApp.get(app);
    if (!g) {
      g = { key: app, appName: app, members: [], totalMemory: 0, totalCpu: 0 };
      byApp.set(app, g);
    }
    g.members.push(p);
    g.totalMemory += p.memoryRss;
    g.totalCpu += p.cpuPercent;
  }

  const grouped: AppGroup[] = [];
  const claimed = new Set<number>();
  for (const g of byApp.values()) {
    if (g.members.length < 2) continue;
    g.members.sort((a, b) => b.memoryRss - a.memoryRss);
    grouped.push(g);
    for (const p of g.members) claimed.add(p.pid);
  }

  const items: AppGroupItem[] = grouped.map((group) => ({ kind: "group", group }));
  for (const p of processes) {
    if (!claimed.has(p.pid)) items.push({ kind: "single", process: p });
  }
  items.sort((a, b) => {
    const am = a.kind === "group" ? a.group.totalMemory : a.process.memoryRss;
    const bm = b.kind === "group" ? b.group.totalMemory : b.process.memoryRss;
    return bm - am;
  });
  return items;
}
