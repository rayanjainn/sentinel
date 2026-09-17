import { DotsThree, Plugs, Prohibit } from "@phosphor-icons/react";
import { useMemo, useRef } from "react";

import type { SocketEntry } from "../../bindings/SocketEntry";
import { Button, IconButton } from "../../components/Button";
import { openContextMenu, openMenuAt } from "../../components/ContextMenu";
import { EmptyState, StatePanel } from "../../components/States";
import { useVirtualRows } from "../../lib/virtual";
import { blockLocalPort, socketMenu } from "./socketActions";

const ROW_HEIGHT = 32;
const HEADER_HEIGHT = 30;
const GRID = "90px 64px minmax(200px,1fr) minmax(220px,1.2fr) 150px 40px";

function bindLabel(s: SocketEntry): { text: string; detail: string | null } {
  const addr = s.localAddr;
  if (addr === "0.0.0.0" || addr === "::" || addr === "*") return { text: "All interfaces", detail: "Reachable from other devices" };
  if (addr.startsWith("127.") || addr === "::1") return { text: "This computer only", detail: addr };
  return { text: addr, detail: null };
}

export function ListeningTable({ sockets }: { sockets: SocketEntry[] }) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const rows = useMemo(
    () => [...sockets].sort((a, b) => a.localPort - b.localPort || a.protocol.localeCompare(b.protocol) || a.family.localeCompare(b.family)),
    [sockets],
  );
  const { start, end, totalHeight } = useVirtualRows(scrollRef, rows.length, ROW_HEIGHT, { headerOffset: HEADER_HEIGHT });

  if (rows.length === 0) {
    return (
      <StatePanel>
        <EmptyState
          icon={<Plugs size={22} />}
          title="No listening ports match"
          detail="Nothing is accepting connections with the current filters."
        />
      </StatePanel>
    );
  }

  return (
    <div ref={scrollRef} role="grid" aria-label="Listening ports" className="h-full overflow-auto">
      <div style={{ minWidth: 820 }}>
        <div
          role="row"
          className="sticky top-0 z-10 grid items-center border-b border-line bg-ground text-[12px] text-fg-muted"
          style={{ gridTemplateColumns: GRID, height: HEADER_HEIGHT }}
        >
          <span role="columnheader" className="px-4 text-right">Port</span>
          <span role="columnheader" className="px-3">Proto</span>
          <span role="columnheader" className="px-3">Listening on</span>
          <span role="columnheader" className="px-3">Process</span>
          <span />
          <span />
        </div>
        <div role="rowgroup" className="relative" style={{ height: totalHeight }}>
          {rows.slice(start, end).map((s, i) => {
            const bind = bindLabel(s);
            return (
              <div
                key={s.id}
                role="row"
                className="group absolute left-0 right-0 grid items-center border-b border-line text-[12px] hover:bg-raised/60"
                style={{ gridTemplateColumns: GRID, height: ROW_HEIGHT, transform: `translateY(${(start + i) * ROW_HEIGHT}px)` }}
                onContextMenu={(e) => openContextMenu(e, socketMenu(s, true), `Port ${s.localPort}`)}
              >
                <span className="num px-4 text-right text-[13px] text-fg">{s.localPort}</span>
                <span className="px-3 text-fg-muted">{s.protocol.toUpperCase()}</span>
                <span className="flex min-w-0 items-baseline gap-2 px-3">
                  <span className={bind.text === s.localAddr ? "num truncate text-fg" : "truncate text-fg"}>{bind.text}</span>
                  {bind.detail && <span className="truncate text-fg-subtle">{bind.detail}</span>}
                </span>
                <span className="flex min-w-0 items-baseline gap-2 px-3">
                  <span className="truncate text-fg">{s.processName ?? "Unknown process"}</span>
                  {s.pid !== null && <span className="num text-fg-subtle">{s.pid}</span>}
                </span>
                <span className="px-3 opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100">
                  <Button size="sm" variant="dangerQuiet" icon={<Prohibit size={13} />} onClick={() => void blockLocalPort(s.localPort, s.protocol)}>
                    Block port
                  </Button>
                </span>
                <span className="flex justify-center">
                  <IconButton label={`Actions for port ${s.localPort}`} size="sm" onClick={(e) => openMenuAt(e.currentTarget, socketMenu(s, true), `Port ${s.localPort}`)}>
                    <DotsThree size={14} weight="bold" />
                  </IconButton>
                </span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
