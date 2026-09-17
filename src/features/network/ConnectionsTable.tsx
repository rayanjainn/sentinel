import { ArrowDown, ArrowUp, CaretRight, DotsThree, GlobeSimple } from "@phosphor-icons/react";
import { motion } from "motion/react";
import { memo, useMemo, useRef } from "react";

import type { SocketEntry } from "../../bindings/SocketEntry";
import type { TrafficSource } from "../../bindings/TrafficSource";
import { IconButton } from "../../components/Button";
import { openContextMenu, openMenuAt } from "../../components/ContextMenu";
import { EmptyState, StatePanel } from "../../components/States";
import { cx } from "../../lib/cx";
import { formatBytes, formatCount, formatRate, pluralize } from "../../lib/format";
import { formatEndpoint, TCP_STATE_LABEL } from "../../lib/net";
import { useVirtualRows } from "../../lib/virtual";
import { hostFor } from "../../stores/network";
import { groupByProcess, type ProcessGroup } from "./grouping";
import { processMenu, socketMenu } from "./socketActions";
import { useNetworkView } from "./viewState";

const ROW_HEIGHT = 28;
const HEADER_HEIGHT = 30;
const GRID = "minmax(200px,1.3fr) minmax(190px,1fr) 80px 52px 96px minmax(130px,0.8fr) 170px 36px";

type Row = { type: "group"; group: ProcessGroup; expanded: boolean } | { type: "socket"; socket: SocketEntry };

export function trafficHeader(source: TrafficSource | null, sockets: SocketEntry[]): { label: string; title: string } {
  if (sockets.some((s) => s.rxBps !== null || s.txBps !== null)) {
    return { label: "Rate", title: "Live per-connection transfer rate reported by the operating system" };
  }
  if (sockets.some((s) => s.bytesIn !== null || s.bytesOut !== null)) {
    return { label: "Transferred", title: "Cumulative bytes: kernel counters where available, otherwise since Sentinel first saw the connection" };
  }
  return {
    label: "Traffic",
    title: source === "interfaceOnly" ? "This system reports traffic per network interface only" : "No per-connection traffic reported",
  };
}

export function trafficNote(source: TrafficSource | null): string | null {
  switch (source) {
    case "perConnection":
      return null;
    case "perProcess":
      return "This system reports traffic per process rather than per connection. Where a connection shows bytes, they were counted since Sentinel first saw it.";
    case "interfaceOnly":
      return "This system only reports traffic per network interface, so connections show no traffic. The graph above covers the whole machine.";
    default:
      return null;
  }
}

function Traffic({ rx, tx, rate }: { rx: number | null; tx: number | null; rate: boolean }) {
  if (rx === null && tx === null) return <span className="text-fg-subtle">—</span>;
  const fmt = rate ? formatRate : (v: number) => formatBytes(v, { base: 1000 });
  return (
    <span className="num flex items-center justify-end gap-2">
      <span className="flex items-center gap-0.5 text-fg">
        <ArrowDown size={10} className="text-fg-subtle" />
        {fmt(rx ?? 0)}
      </span>
      <span className="flex items-center gap-0.5 text-fg-muted">
        <ArrowUp size={10} className="text-fg-subtle" />
        {fmt(tx ?? 0)}
      </span>
    </span>
  );
}

const GroupRow = memo(function GroupRow({
  group,
  expanded,
  index,
  rate,
}: {
  group: ProcessGroup;
  expanded: boolean;
  index: number;
  rate: boolean;
}) {
  const toggle = () => useNetworkView.getState().toggleGroup(group.key);
  const hasTraffic = rate ? group.rxBps + group.txBps > 0 : group.bytesIn + group.bytesOut > 0;
  return (
    <div
      role="row"
      aria-expanded={expanded}
      className="absolute left-0 right-0 grid cursor-default items-center border-b border-line bg-ground text-[12px] hover:bg-raised/60"
      style={{ gridTemplateColumns: GRID, height: ROW_HEIGHT, transform: `translateY(${index * ROW_HEIGHT}px)` }}
      onClick={toggle}
      onContextMenu={(e) => openContextMenu(e, processMenu(group.pid, group.name), `Actions for ${group.name}`)}
    >
      <div className="col-span-6 flex min-w-0 items-center gap-2 pl-2.5">
        <CaretRight size={10} weight="bold" className={cx("shrink-0 text-fg-muted transition-transform duration-150", expanded && "rotate-90")} />
        <span className="truncate font-medium text-fg">{group.name}</span>
        {group.pid !== null && <span className="num shrink-0 text-fg-subtle">{group.pid}</span>}
        <span className="shrink-0 text-fg-muted">{pluralize(group.sockets.length, "connection")}</span>
      </div>
      <div className="px-3 text-right">
        {hasTraffic ? (
          <Traffic rx={rate ? group.rxBps : group.bytesIn} tx={rate ? group.txBps : group.bytesOut} rate={rate} />
        ) : (
          <span className="text-fg-subtle">—</span>
        )}
      </div>
      <div className="flex justify-center">
        <IconButton
          label={`Actions for ${group.name}`}
          size="sm"
          onClick={(e) => {
            e.stopPropagation();
            openMenuAt(e.currentTarget, processMenu(group.pid, group.name), `Actions for ${group.name}`);
          }}
        >
          <DotsThree size={14} weight="bold" />
        </IconButton>
      </div>
    </div>
  );
});

const SocketRow = memo(function SocketRow({
  socket: s,
  host,
  index,
  selected,
  rate,
}: {
  socket: SocketEntry;
  host: string | null;
  index: number;
  selected: boolean;
  rate: boolean;
}) {
  const place = s.geo ? [s.geo.city, s.geo.country].filter(Boolean).join(", ") : null;
  const scopeNote =
    s.remoteScope === "loopback" ? "This computer" : s.remoteScope === "private" || s.remoteScope === "linkLocal" ? "Local network" : null;
  return (
    <div
      role="row"
      aria-selected={selected}
      className={cx(
        "absolute left-0 right-0 grid items-center text-[12px]",
        selected ? "bg-signal/12" : "hover:bg-raised/60",
      )}
      style={{ gridTemplateColumns: GRID, height: ROW_HEIGHT, transform: `translateY(${index * ROW_HEIGHT}px)` }}
      onClick={() => useNetworkView.getState().selectSocket(selected ? null : s.id)}
      onContextMenu={(e) => {
        useNetworkView.getState().selectSocket(s.id);
        openContextMenu(e, socketMenu(s, false), "Connection actions");
      }}
    >
      {selected && <span className="absolute inset-y-0 left-0 w-0.5 bg-signal" />}
      <div className="min-w-0 truncate pl-8 pr-3">
        {host ? (
          <motion.span key={host} initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ duration: 0.4 }} className="text-fg" title={host}>
            {host}
          </motion.span>
        ) : (
          <span className="text-fg-subtle">{s.remoteAddr ? "Resolving…" : "—"}</span>
        )}
      </div>
      <div className="num truncate px-3 text-fg" title={s.remoteAddr ?? undefined}>
        {s.remoteAddr ? formatEndpoint(s.remoteAddr, s.remotePort, s.family) : "—"}
      </div>
      <div className="num px-3 text-right text-fg-muted">{s.localPort}</div>
      <div className="px-3 text-fg-muted">{s.protocol.toUpperCase()}</div>
      <div className="truncate px-3 text-fg-muted">{s.state ? TCP_STATE_LABEL[s.state] : "—"}</div>
      <div className="truncate px-3 text-fg-muted" title={place ?? undefined}>
        {place ?? scopeNote ?? <span className="text-fg-subtle">Unknown</span>}
      </div>
      <div className="px-3 text-right">
        <Traffic rx={rate ? s.rxBps : s.bytesIn} tx={rate ? s.txBps : s.bytesOut} rate={rate} />
      </div>
      <div className="flex justify-center">
        <IconButton
          label="Connection actions"
          size="sm"
          onClick={(e) => {
            e.stopPropagation();
            openMenuAt(e.currentTarget, socketMenu(s, false), "Connection actions");
          }}
        >
          <DotsThree size={14} weight="bold" />
        </IconButton>
      </div>
    </div>
  );
});

export function ConnectionsTable({
  sockets,
  trafficSource,
  hostnames,
  forceExpand,
}: {
  sockets: SocketEntry[];
  trafficSource: TrafficSource | null;
  hostnames: Map<string, string | null>;
  /** Expand every group, e.g. while the table is narrowed to a map selection or a search. */
  forceExpand: boolean;
}) {
  const expandedGroups = useNetworkView((s) => s.expandedGroups);
  const selectedSocketId = useNetworkView((s) => s.selectedSocketId);
  const scrollRef = useRef<HTMLDivElement>(null);
  const header = trafficHeader(trafficSource, sockets);
  const rate = header.label === "Rate";

  const rows = useMemo<Row[]>(() => {
    const out: Row[] = [];
    for (const group of groupByProcess(sockets)) {
      const expanded = forceExpand || expandedGroups.has(group.key);
      out.push({ type: "group", group, expanded });
      if (expanded) for (const socket of group.sockets) out.push({ type: "socket", socket });
    }
    return out;
  }, [sockets, expandedGroups, forceExpand]);

  const { start, end, totalHeight } = useVirtualRows(scrollRef, rows.length, ROW_HEIGHT, { headerOffset: HEADER_HEIGHT });

  if (sockets.length === 0) {
    return (
      <StatePanel>
        <EmptyState
          icon={<GlobeSimple size={22} />}
          title="No connections match"
          detail="Change the search or filters above, or clear the map selection."
        />
      </StatePanel>
    );
  }

  return (
    <div ref={scrollRef} role="treegrid" aria-label="Connections by process" className="h-full overflow-auto">
      <div style={{ minWidth: 1000 }}>
        <div
          role="row"
          className="sticky top-0 z-10 grid items-center border-b border-line bg-ground text-[12px] text-fg-muted"
          style={{ gridTemplateColumns: GRID, height: HEADER_HEIGHT }}
        >
          <span role="columnheader" className="pl-8 pr-3">Remote host</span>
          <span role="columnheader" className="px-3">Remote address</span>
          <span role="columnheader" className="px-3 text-right">Local port</span>
          <span role="columnheader" className="px-3">Proto</span>
          <span role="columnheader" className="px-3">State</span>
          <span role="columnheader" className="px-3">Location</span>
          <span role="columnheader" className="px-3 text-right" title={header.title}>
            {header.label}
          </span>
          <span />
        </div>
        <div role="rowgroup" className="relative" style={{ height: totalHeight }}>
          {rows.slice(start, end).map((row, i) => {
            const index = start + i;
            return row.type === "group" ? (
              <GroupRow key={`g-${row.group.key}`} group={row.group} expanded={row.expanded} index={index} rate={rate} />
            ) : (
              <SocketRow
                key={row.socket.id}
                socket={row.socket}
                host={hostFor(row.socket, hostnames)}
                index={index}
                selected={row.socket.id === selectedSocketId}
                rate={rate}
              />
            );
          })}
        </div>
      </div>
    </div>
  );
}

export function connectionSummary(sockets: SocketEntry[]): string {
  const processes = new Set(sockets.map((s) => s.pid ?? -1)).size;
  return `${formatCount(sockets.length)} connections from ${pluralize(processes, "process", "processes")}`;
}
