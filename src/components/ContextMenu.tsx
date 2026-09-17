import { AnimatePresence, motion } from "motion/react";
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { create } from "zustand";

import { cx } from "../lib/cx";

export type MenuItem =
  | { type?: "item"; label: string; icon?: ReactNode; onSelect: () => void; danger?: boolean; disabled?: boolean; hint?: string }
  | { type: "separator" };

interface MenuState {
  menu: { x: number; y: number; items: MenuItem[]; label: string } | null;
  open: (x: number, y: number, items: MenuItem[], label: string) => void;
  close: () => void;
}

export const useContextMenu = create<MenuState>((set) => ({
  menu: null,
  open: (x, y, items, label) => set({ menu: { x, y, items, label } }),
  close: () => set({ menu: null }),
}));

export function openContextMenu(event: { clientX: number; clientY: number; preventDefault: () => void }, items: MenuItem[], label: string) {
  event.preventDefault();
  useContextMenu.getState().open(event.clientX, event.clientY, items, label);
}

/** Menu at an explicit position, e.g. below a "More" button. */
export function openMenuAt(element: HTMLElement, items: MenuItem[], label: string) {
  const rect = element.getBoundingClientRect();
  useContextMenu.getState().open(rect.left, rect.bottom + 4, items, label);
}

export function ContextMenuHost() {
  const menu = useContextMenu((s) => s.menu);
  const close = useContextMenu((s) => s.close);
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  const [active, setActive] = useState(-1);

  useLayoutEffect(() => {
    if (!menu || !ref.current) return;
    const { width, height } = ref.current.getBoundingClientRect();
    const left = Math.min(menu.x, window.innerWidth - width - 8);
    const top = menu.y + height > window.innerHeight - 8 ? Math.max(8, menu.y - height) : menu.y;
    setPos({ left, top });
    ref.current.focus();
  }, [menu]);

  useEffect(() => {
    if (!menu) return;
    const onDown = (event: MouseEvent) => {
      if (!ref.current?.contains(event.target as Node)) close();
    };
    const onBlur = () => close();
    window.addEventListener("mousedown", onDown, true);
    window.addEventListener("blur", onBlur);
    window.addEventListener("resize", onBlur);
    return () => {
      window.removeEventListener("mousedown", onDown, true);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("resize", onBlur);
    };
  }, [menu, close]);

  const selectable = menu?.items.map((item, i) => (item.type !== "separator" && !item.disabled ? i : -1)).filter((i) => i >= 0) ?? [];

  return createPortal(
    <AnimatePresence onExitComplete={() => setPos(null)}>
      {menu && (
        <motion.div
          ref={ref}
          role="menu"
          aria-label={menu.label}
          tabIndex={-1}
          initial={{ opacity: 0, scale: 0.97 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, transition: { duration: 0.08 } }}
          transition={{ duration: 0.12, ease: [0.16, 1, 0.3, 1] }}
          style={{ left: pos?.left ?? menu.x, top: pos?.top ?? menu.y, transformOrigin: "top left" }}
          className="shadow-float fixed z-[70] min-w-52 rounded-[8px] bg-raised p-1 outline-none"
          onKeyDown={(event) => {
            if (event.key === "Escape") close();
            if (event.key === "ArrowDown" || event.key === "ArrowUp") {
              event.preventDefault();
              const idx = selectable.indexOf(active);
              const next = event.key === "ArrowDown" ? (idx + 1) % selectable.length : (idx - 1 + selectable.length) % selectable.length;
              setActive(selectable[next] ?? -1);
            }
            if (event.key === "Enter" && active >= 0) {
              const item = menu.items[active];
              if (item && item.type !== "separator") {
                close();
                item.onSelect();
              }
            }
          }}
        >
          {menu.items.map((item, i) =>
            item.type === "separator" ? (
              <div key={`sep-${i}`} className="mx-2 my-1 h-px bg-line" />
            ) : (
              <button
                key={item.label}
                type="button"
                role="menuitem"
                disabled={item.disabled}
                onMouseEnter={() => setActive(i)}
                onClick={() => {
                  close();
                  item.onSelect();
                }}
                className={cx(
                  "flex h-7 w-full items-center gap-2 rounded-[5px] px-2 text-left text-[13px] disabled:opacity-40",
                  active === i && (item.danger ? "bg-danger/15" : "bg-line-strong"),
                  item.danger ? "text-danger" : "text-fg",
                )}
              >
                <span className="flex w-4 justify-center text-fg-muted">{item.icon}</span>
                <span className="flex-1">{item.label}</span>
                {item.hint && <span className="text-[11px] text-fg-subtle">{item.hint}</span>}
              </button>
            ),
          )}
        </motion.div>
      )}
    </AnimatePresence>,
    document.body,
  );
}
