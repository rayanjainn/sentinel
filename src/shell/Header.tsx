import { SidebarSimple } from "@phosphor-icons/react";

import { IconButton } from "../components/Button";
import { modKey } from "../lib/platform";
import { useSettings } from "../stores/settings";
import { useHeaderSlotTarget } from "./HeaderSlot";
import { LiveIndicator } from "./LiveIndicator";
import { NAV_ITEMS } from "./navigation";

/** Unified title bar: draggable, hosts the active view's title and toolbar. */
export function Header() {
  const view = useSettings((s) => s.view);
  const panelOpen = useSettings((s) => s.panelOpen);
  const togglePanel = useSettings((s) => s.togglePanel);
  const setSlot = useHeaderSlotTarget();
  const title = NAV_ITEMS.find((n) => n.id === view)?.label ?? "";

  return (
    <header data-tauri-drag-region className="flex h-[52px] shrink-0 items-center gap-4 border-b border-line bg-ground pl-5 pr-3">
      <h1 data-tauri-drag-region className="shrink-0 text-[15px] font-semibold tracking-display text-fg">
        {title}
      </h1>
      <div ref={setSlot} data-tauri-drag-region className="flex min-w-0 flex-1 items-center gap-2" />
      <LiveIndicator />
      <div className="h-4 w-px bg-line" />
      <IconButton
        label={`${panelOpen ? "Hide" : "Show"} agent panel (${modKey}J)`}
        active={panelOpen}
        onClick={togglePanel}
      >
        <SidebarSimple size={16} weight={panelOpen ? "fill" : "regular"} className="-scale-x-100" />
      </IconButton>
    </header>
  );
}
