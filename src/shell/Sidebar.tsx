import { motion } from "motion/react";
import { useRef, type KeyboardEvent } from "react";

import { cx } from "../lib/cx";
import { springInteraction } from "../lib/motion";
import { isMac, modKey } from "../lib/platform";
import { useResources } from "../stores/resources";
import { useSettings } from "../stores/settings";
import { NAV_ITEMS } from "./navigation";

export function Sidebar() {
  const view = useSettings((s) => s.view);
  const setView = useSettings((s) => s.setView);
  const hostname = useResources((s) => s.systemInfo?.hostname ?? null);
  const osName = useResources((s) => (s.systemInfo ? `${s.systemInfo.osName} ${s.systemInfo.osVersion ?? ""}`.trim() : null));
  const refs = useRef<Array<HTMLButtonElement | null>>([]);

  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    const index = refs.current.findIndex((el) => el === document.activeElement);
    const next = (index + (event.key === "ArrowDown" ? 1 : -1) + NAV_ITEMS.length) % NAV_ITEMS.length;
    refs.current[next]?.focus();
  };

  return (
    <aside className="flex h-full w-[212px] shrink-0 flex-col border-r border-line bg-panel">
      <div data-tauri-drag-region className={cx("flex shrink-0 items-end px-4", isMac ? "h-[52px] pb-2" : "h-12 pb-3")}>
        {!isMac && <Wordmark />}
      </div>
      {isMac && (
        <div data-tauri-drag-region className="px-4 pb-3">
          <Wordmark />
        </div>
      )}
      <nav aria-label="Views" className="flex flex-1 flex-col gap-0.5 px-2" onKeyDown={onKeyDown}>
        {NAV_ITEMS.map((item, i) => {
          const active = item.id === view;
          const Icon = item.icon;
          return (
            <div key={item.id} className={cx(item.group === "app" && i > 0 && NAV_ITEMS[i - 1]?.group === "monitor" && "mt-4")}>
              <button
                ref={(el) => {
                  refs.current[i] = el;
                }}
                type="button"
                aria-current={active ? "page" : undefined}
                onClick={() => setView(item.id)}
                className={cx(
                  "group relative flex h-8 w-full items-center gap-2.5 rounded-[6px] px-2.5 text-left transition-colors",
                  active ? "text-fg" : "text-fg-muted hover:bg-raised/60 hover:text-fg",
                )}
              >
                {active && (
                  <motion.span
                    layoutId="nav-active"
                    transition={springInteraction}
                    className="absolute inset-0 rounded-[6px] bg-raised"
                  />
                )}
                <Icon size={16} weight={active ? "fill" : "regular"} className={cx("relative", active && "text-signal")} />
                <span className="relative flex-1">{item.label}</span>
                <kbd className="relative font-sans text-[11px] text-fg-subtle opacity-0 transition-opacity group-hover:opacity-100">
                  {modKey}
                  {i + 1}
                </kbd>
              </button>
            </div>
          );
        })}
      </nav>
      <div className="flex flex-col gap-0.5 border-t border-line px-4 py-3">
        <span className="truncate text-[12px] text-fg">{hostname ?? "This computer"}</span>
        <span className="truncate text-[11px] text-fg-subtle">{osName ?? "System details unavailable"}</span>
      </div>
    </aside>
  );
}

function Wordmark() {
  return (
    <div className="flex items-center gap-2">
      <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden className="text-signal">
        <circle cx="8" cy="8" r="6.5" fill="none" stroke="currentColor" strokeOpacity="0.35" strokeWidth="1.2" />
        <circle cx="8" cy="8" r="3.5" fill="none" stroke="currentColor" strokeOpacity="0.7" strokeWidth="1.2" />
        <circle cx="8" cy="8" r="1.4" fill="currentColor" />
      </svg>
      <span className="text-[13px] font-semibold tracking-display text-fg">Sentinel</span>
    </div>
  );
}
