// Shared floating-tooltip mechanics for InfoTip (static glossary) and any per-row dynamic
// explanation (e.g. a connection's plain-language explanation): a real button, keyboard-focusable,
// that shows a small panel on hover, focus and tap.
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

import { cx } from "../lib/cx";
import { usePrefersReducedMotion } from "../lib/motion";

const MARGIN = 8;
const WIDTH = 260;

export function Tip({
  label,
  content,
  icon,
  className,
  panelClassName,
  width = WIDTH,
}: {
  /** Accessible name for the trigger button. */
  label: string;
  content: ReactNode;
  icon: ReactNode;
  className?: string;
  panelClassName?: string;
  /** Panel width in pixels; also used to keep it inside the viewport. */
  width?: number;
}) {
  const buttonRef = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<{ left: number; top: number; above: boolean } | null>(null);
  const panelId = useId();
  const reduced = usePrefersReducedMotion();

  useEffect(() => {
    if (!open) return;
    const button = buttonRef.current;
    if (!button) return;
    const place = () => {
      const rect = button.getBoundingClientRect();
      const above = rect.top > window.innerHeight / 2;
      setPos({
        left: Math.min(Math.max(MARGIN, rect.left + rect.width / 2 - width / 2), window.innerWidth - width - MARGIN),
        top: above ? rect.top - MARGIN : rect.bottom + MARGIN,
        above,
      });
    };
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [open, width]);

  return (
    <span className={cx("relative inline-flex", className)}>
      <button
        ref={buttonRef}
        type="button"
        aria-label={label}
        aria-describedby={open ? panelId : undefined}
        aria-expanded={open}
        onMouseEnter={() => setOpen(true)}
        onMouseLeave={() => setOpen(false)}
        onFocus={() => setOpen(true)}
        onBlur={() => setOpen(false)}
        onClick={(e) => {
          e.stopPropagation();
          e.preventDefault();
          setOpen((v) => !v);
        }}
        onKeyDown={(e) => {
          if (e.key === "Escape") setOpen(false);
        }}
        className="inline-flex size-3.5 shrink-0 items-center justify-center rounded-full text-fg-subtle transition-colors hover:text-fg focus-visible:text-fg"
      >
        {icon}
      </button>
      {createPortal(
        <AnimatePresence>
          {open && pos && (
            <motion.div
              id={panelId}
              role="tooltip"
              initial={reduced ? { opacity: 0 } : { opacity: 0, y: pos.above ? 4 : -4, scale: 0.98 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={reduced ? { opacity: 0 } : { opacity: 0, y: pos.above ? 4 : -4, scale: 0.98 }}
              transition={{ duration: reduced ? 0 : 0.12, ease: [0.16, 1, 0.3, 1] }}
              style={{
                left: pos.left,
                top: pos.above ? undefined : pos.top,
                bottom: pos.above ? window.innerHeight - pos.top : undefined,
                width,
              }}
              className={cx(
                "shadow-float pointer-events-none fixed z-[80] flex flex-col gap-1 rounded-[8px] bg-raised px-3 py-2.5 text-[12px]",
                panelClassName,
              )}
            >
              {content}
            </motion.div>
          )}
        </AnimatePresence>,
        document.body,
      )}
    </span>
  );
}
