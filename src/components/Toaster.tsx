import { CheckCircle, Info, WarningCircle, X } from "@phosphor-icons/react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";

import { cx } from "../lib/cx";
import { springInteraction, usePrefersReducedMotion } from "../lib/motion";
import { useToasts, type Toast } from "../stores/toasts";

function ToastItem({ toast }: { toast: Toast }) {
  const dismiss = useToasts((s) => s.dismiss);
  const [hovered, setHovered] = useState(false);
  const reduced = usePrefersReducedMotion();

  useEffect(() => {
    if (hovered) return;
    const timer = window.setTimeout(() => dismiss(toast.id), toast.durationMs);
    return () => window.clearTimeout(timer);
  }, [hovered, dismiss, toast.id, toast.durationMs]);

  const Icon = toast.kind === "success" ? CheckCircle : toast.kind === "error" ? WarningCircle : Info;
  return (
    <motion.div
      layout={!reduced}
      role={toast.kind === "error" ? "alert" : "status"}
      initial={reduced ? { opacity: 0 } : { opacity: 0, y: 12, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={reduced ? { opacity: 0 } : { opacity: 0, x: 24 }}
      transition={reduced ? { duration: 0 } : springInteraction}
      onHoverStart={() => setHovered(true)}
      onHoverEnd={() => setHovered(false)}
      className="shadow-float pointer-events-auto flex w-[360px] items-start gap-3 rounded-[8px] bg-raised px-3.5 py-3"
    >
      <Icon
        size={16}
        weight="fill"
        className={cx(
          "mt-0.5 shrink-0",
          toast.kind === "success" && "text-signal",
          toast.kind === "error" && "text-danger",
          toast.kind === "info" && "text-fg-muted",
        )}
      />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <p className="font-medium text-fg">{toast.title}</p>
        {toast.detail && <div className="text-[12px] text-fg-muted">{toast.detail}</div>}
        {toast.action && (
          <button
            type="button"
            className="mt-1 self-start text-[12px] font-medium text-signal hover:underline"
            onClick={() => {
              toast.action?.onClick();
              dismiss(toast.id);
            }}
          >
            {toast.action.label}
          </button>
        )}
      </div>
      <button
        type="button"
        aria-label="Dismiss"
        className="-mr-1 rounded-[4px] p-0.5 text-fg-subtle hover:text-fg"
        onClick={() => dismiss(toast.id)}
      >
        <X size={13} />
      </button>
    </motion.div>
  );
}

export function Toaster() {
  const toasts = useToasts((s) => s.toasts);
  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-[60] flex flex-col items-end gap-2" aria-live="polite">
      <AnimatePresence initial={false}>
        {toasts.map((t) => (
          <ToastItem key={t.id} toast={t} />
        ))}
      </AnimatePresence>
    </div>
  );
}
