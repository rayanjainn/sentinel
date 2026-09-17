import { AnimatePresence, motion } from "motion/react";
import { useRef, type PointerEvent } from "react";

import { AgentPanel } from "../features/agent";
import { springPanel, usePrefersReducedMotion } from "../lib/motion";
import { clampPanelWidth, PANEL_MAX_WIDTH, PANEL_MIN_WIDTH, useSettings } from "../stores/settings";

/** Right-hand dock hosting the agent panel. Resizable from its leading edge. */
export function AgentDock() {
  const open = useSettings((s) => s.panelOpen);
  const width = useSettings((s) => s.panelWidth);
  const setWidth = useSettings((s) => s.setPanelWidth);
  const setOpen = useSettings((s) => s.setPanelOpen);
  const reduced = usePrefersReducedMotion();
  const dockRef = useRef<HTMLElement>(null);
  const drag = useRef<{ startX: number; startWidth: number } | null>(null);

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { startX: event.clientX, startWidth: width };
  };
  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (!drag.current || !dockRef.current) return;
    const next = clampPanelWidth(drag.current.startWidth + (drag.current.startX - event.clientX));
    dockRef.current.style.width = `${next}px`;
  };
  const onPointerUp = (event: PointerEvent<HTMLDivElement>) => {
    if (!drag.current) return;
    const next = clampPanelWidth(drag.current.startWidth + (drag.current.startX - event.clientX));
    drag.current = null;
    setWidth(next);
  };

  return (
    <AnimatePresence initial={false}>
      {open && (
        <motion.aside
          ref={dockRef}
          key="agent-dock"
          aria-label="Agent"
          className="relative flex h-full shrink-0 flex-col border-l border-line bg-panel"
          style={{ width }}
          initial={reduced ? { opacity: 0 } : { x: 48, opacity: 0 }}
          animate={{ x: 0, opacity: 1 }}
          exit={reduced ? { opacity: 0 } : { x: 48, opacity: 0 }}
          transition={reduced ? { duration: 0 } : springPanel}
        >
          <div
            role="separator"
            aria-orientation="vertical"
            aria-label="Resize agent panel"
            aria-valuemin={PANEL_MIN_WIDTH}
            aria-valuemax={PANEL_MAX_WIDTH}
            aria-valuenow={width}
            tabIndex={0}
            onKeyDown={(event) => {
              if (event.key === "ArrowLeft") setWidth(width + 16);
              if (event.key === "ArrowRight") setWidth(width - 16);
            }}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            className="absolute inset-y-0 -left-1 z-10 w-2 cursor-col-resize after:absolute after:inset-y-0 after:left-[3px] after:w-px after:bg-signal after:opacity-0 after:transition-opacity hover:after:opacity-60 focus-visible:after:opacity-100"
          />
          <div className="flex min-h-0 flex-1 flex-col">
            <AgentPanel onClose={() => setOpen(false)} />
          </div>
        </motion.aside>
      )}
    </AnimatePresence>
  );
}
