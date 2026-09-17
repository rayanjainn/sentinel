import { Info, Lightbulb } from "@phosphor-icons/react";
import { useMemo } from "react";

import type { CleanupSuggestion } from "../../bindings/CleanupSuggestion";
import { openContextMenu } from "../../components/ContextMenu";
import { Checkbox } from "../../components/Field";
import { InfoTip } from "../../components/InfoTip";
import { EmptyState, StatePanel } from "../../components/States";
import { cx } from "../../lib/cx";
import { formatBytes, formatCount, formatRelativeSecs } from "../../lib/format";
import { useStorage } from "../../stores/storage";
import { CATEGORY_GLOSSARY, CATEGORY_LABEL, CATEGORY_ORDER } from "./categories";
import { SelectionBar } from "./SelectionBar";
import { summarize, toggle } from "./selection";
import { moveItems, pathMenu, trashItems } from "./storageActions";
import { isHidden } from "./treemap";
import { useStorageView } from "./viewState";

function lastUsed(s: CleanupSuggestion): number | null {
  if (s.lastModified === null && s.lastAccessed === null) return null;
  return Math.max(s.lastModified ?? 0, s.lastAccessed ?? 0);
}

export function Suggestions() {
  const summary = useStorage((s) => s.summary);
  const removed = useStorage((s) => s.removed);
  const selected = useStorageView((s) => s.suggestionSelected);
  const setSelected = useStorageView((s) => s.setSuggestionSelected);

  const groups = useMemo(() => {
    const visible = (summary?.suggestions ?? []).filter((s) => !isHidden(s.path, removed));
    return CATEGORY_ORDER.map((category) => {
      const items = visible.filter((s) => s.category === category).sort((a, b) => b.sizeBytes - a.sizeBytes);
      return { category, items, bytes: items.reduce((sum, i) => sum + i.sizeBytes, 0) };
    }).filter((g) => g.items.length > 0);
  }, [summary, removed]);

  const chosen = groups.flatMap((g) => g.items).filter((s) => s.actionable && selected.has(s.id));
  const { count, bytes } = summarize(chosen);

  if (groups.length === 0) {
    return (
      <StatePanel>
        <EmptyState
          icon={<Lightbulb size={22} />}
          title="No cleanup suggestions for this scan"
          detail="Sentinel looks for caches, logs, old downloads and build output that usually regenerate on their own."
        />
      </StatePanel>
    );
  }

  return (
    <div className="relative h-full">
      <div className="h-full overflow-y-auto pb-24">
        <div className="mx-auto flex max-w-[1000px] flex-col gap-6 px-6 py-5">
          <p className="flex items-start gap-2 text-fg-muted">
            <Info size={15} className="mt-0.5 shrink-0" />
            These are suggestions, not instructions. Each one explains why it is usually safe to remove and what
            recreates it. Anything you choose goes to the Trash, where it can be put back.
          </p>
          {groups.map((g) => {
            const actionable = g.items.filter((i) => i.actionable);
            const allOn = actionable.length > 0 && actionable.every((i) => selected.has(i.id));
            const someOn = actionable.some((i) => selected.has(i.id));
            return (
              <section key={g.category} aria-label={CATEGORY_LABEL[g.category]} className="flex flex-col">
                <div className="flex items-center gap-3 border-b border-line pb-2">
                  {actionable.length > 0 ? (
                    <Checkbox
                      label={`Select all ${CATEGORY_LABEL[g.category]}`}
                      checked={allOn}
                      indeterminate={someOn && !allOn}
                      onChange={(on) => {
                        let next = new Set(selected);
                        for (const i of actionable) next = toggle(next, i.id, on);
                        setSelected(next);
                      }}
                    />
                  ) : (
                    <span className="w-3.5" />
                  )}
                  <h3 className="flex flex-1 items-center gap-1.5 text-[15px] font-semibold tracking-display text-fg">
                    {CATEGORY_LABEL[g.category]}
                    <InfoTip id={CATEGORY_GLOSSARY[g.category]} />
                  </h3>
                  <span className="num text-fg">{formatBytes(g.bytes, { base: 1000 })}</span>
                </div>
                <ul className="flex flex-col divide-y divide-line">
                  {g.items.map((s) => {
                    const on = selected.has(s.id);
                    const used = lastUsed(s);
                    return (
                      <li
                        key={s.id}
                        className={cx("grid grid-cols-[14px_minmax(0,1fr)_100px] gap-3 py-2.5", on && "bg-signal/5")}
                        onContextMenu={(e) => openContextMenu(e, pathMenu(s), s.path)}
                      >
                        {s.actionable ? (
                          <Checkbox label={`Select ${s.path}`} checked={on} onChange={(v) => setSelected(toggle(selected, s.id, v))} className="mt-0.5" />
                        ) : (
                          <span />
                        )}
                        <div className="flex min-w-0 flex-col gap-1">
                          <span className="num selectable truncate text-[12px] text-fg" title={s.path}>
                            {s.path}
                          </span>
                          <span className="text-[12px] text-fg-muted">{s.rationale}</span>
                          <span className="flex flex-wrap gap-x-4 text-[11px] text-fg-subtle">
                            <span>{formatCount(s.itemCount)} items</span>
                            {used !== null && <span>Last used {formatRelativeSecs(used)}</span>}
                            {!s.actionable && <span className="text-fg-muted">Shown for information only. Sentinel will not remove this.</span>}
                          </span>
                        </div>
                        <span className="num text-right text-fg">{formatBytes(s.sizeBytes, { base: 1000 })}</span>
                      </li>
                    );
                  })}
                </ul>
              </section>
            );
          })}
        </div>
      </div>
      <SelectionBar
        count={count}
        bytes={bytes}
        onClear={() => setSelected(new Set())}
        onTrash={() => void trashItems(chosen).then((ok) => ok && setSelected(new Set()))}
        onMove={() => void moveItems(chosen).then((ok) => ok && setSelected(new Set()))}
      />
    </div>
  );
}
