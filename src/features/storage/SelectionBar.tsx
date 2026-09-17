import { FolderOpen, Trash } from "@phosphor-icons/react";
import { AnimatePresence, motion } from "motion/react";
import type { ReactNode } from "react";

import { Button } from "../../components/Button";
import { InfoTip } from "../../components/InfoTip";
import { formatBytes, pluralize } from "../../lib/format";
import { springInteraction } from "../../lib/motion";

/** Summary of a batch selection with the two storage actions; appears only while something is selected. */
export function SelectionBar({
  count,
  bytes,
  onTrash,
  onMove,
  onClear,
  warning,
  disabled = false,
}: {
  count: number;
  bytes: number;
  onTrash: () => void;
  onMove: () => void;
  onClear: () => void;
  warning?: ReactNode;
  disabled?: boolean;
}) {
  return (
    <AnimatePresence>
      {count > 0 && (
        <motion.div
          initial={{ y: 16, opacity: 0 }}
          animate={{ y: 0, opacity: 1 }}
          exit={{ y: 16, opacity: 0 }}
          transition={springInteraction}
          className="shadow-float absolute bottom-4 left-1/2 z-10 flex -translate-x-1/2 items-center gap-4 rounded-[10px] bg-raised py-2 pl-4 pr-2"
          role="region"
          aria-label="Selection"
        >
          <div className="flex flex-col">
            <span className="whitespace-nowrap text-fg">
              {pluralize(count, "item")} selected, <span className="num">{formatBytes(bytes, { base: 1000 })}</span>
            </span>
            {warning && <span className="text-[12px] text-warn">{warning}</span>}
          </div>
          <div className="flex items-center gap-1.5">
            <Button variant="ghost" size="sm" onClick={onClear}>
              Clear
            </Button>
            <Button size="sm" icon={<FolderOpen size={13} />} onClick={onMove} disabled={disabled}>
              Move to…
            </Button>
            <Button size="sm" variant="danger" icon={<Trash size={13} />} onClick={onTrash} disabled={disabled}>
              Move to Trash
            </Button>
            <InfoTip id="moveToTrash" />
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
