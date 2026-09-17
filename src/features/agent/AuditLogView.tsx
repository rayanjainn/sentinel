import { ArrowsClockwise, CaretRight, FolderOpen } from "@phosphor-icons/react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";

import type { AuditEntry } from "../../bindings/AuditEntry";
import type { AuditStatus } from "../../bindings/AuditStatus";
import type { OriginFilter } from "../../bindings/OriginFilter";
import { Button, IconButton } from "../../components/Button";
import { Segmented } from "../../components/Segmented";
import { SkeletonRows } from "../../components/Skeleton";
import { EmptyState, ErrorState, StatePanel } from "../../components/States";
import { describeError } from "../../lib/errors";
import { cx } from "../../lib/cx";
import { formatBytes, formatDateTime, formatMetric } from "../../lib/format";
import { api, on } from "../../lib/ipc";
import { springInteraction, usePrefersReducedMotion } from "../../lib/motion";

const PAGE_SIZE = 50;
const COLUMNS = "grid-cols-[148px_minmax(0,1.2fr)_minmax(0,2fr)_128px_84px_20px]";

const STATUS: Record<AuditStatus, { label: string; className: string }> = {
  succeeded: { label: "Succeeded", className: "text-signal" },
  partiallySucceeded: { label: "Partly succeeded", className: "text-warn" },
  failed: { label: "Failed", className: "text-danger" },
  rejected: { label: "Declined", className: "text-fg-subtle" },
};

function originLabel(entry: AuditEntry): { short: string; full: string } {
  if (entry.origin.type === "user") return { short: "You", full: "You, from the app" };
  const { provider, model } = entry.origin;
  return { short: `Agent, ${model}`, full: `Agent using ${provider} (${model})` };
}

function RevealButton({ path }: { path: string }) {
  const [problem, setProblem] = useState<string | null>(null);
  return (
    <span className="inline-flex items-center gap-2">
      <Button
        size="sm"
        variant="ghost"
        icon={<FolderOpen size={13} />}
        onClick={() => {
          setProblem(null);
          api.revealPath(path).catch((e: unknown) => setProblem(describeError(e).title));
        }}
      >
        Reveal
      </Button>
      {problem && <span className="text-[12px] text-fg-subtle">{problem}</span>}
    </span>
  );
}

function EntryDetails({ entry }: { entry: AuditEntry }) {
  const keys = [...new Set([...entry.before.map((m) => m.key), ...entry.after.map((m) => m.key)])];
  return (
    <div className="flex flex-col gap-3 border-t border-line bg-sunken/50 px-3 py-3 text-[12px]">
      {entry.trigger && (
        <div className="flex flex-col gap-0.5">
          <span className="text-fg-subtle">Request</span>
          <span className="selectable text-fg">{entry.trigger}</span>
        </div>
      )}
      <div className="flex flex-col gap-0.5">
        <span className="text-fg-subtle">Result</span>
        <span className="selectable text-fg">{entry.summary}</span>
      </div>
      {keys.length > 0 && (
        <div className="flex flex-col gap-1">
          <span className="text-fg-subtle">Measured before and after</span>
          <ul className="flex flex-col gap-0.5">
            {keys.map((key) => {
              const before = entry.before.find((m) => m.key === key);
              const after = entry.after.find((m) => m.key === key);
              return (
                <li key={key} className="flex flex-wrap items-baseline gap-2">
                  <span className="text-fg-muted">{(before ?? after)?.label}</span>
                  <span className="num text-fg">{before ? formatMetric(before) : "not measured"}</span>
                  <span className="text-fg-subtle">to</span>
                  <span className="num text-fg">{after ? formatMetric(after) : "not measured"}</span>
                </li>
              );
            })}
          </ul>
        </div>
      )}
      {entry.affectedPaths.length > 0 && (
        <div className="flex flex-col gap-1">
          <span className="text-fg-subtle">
            {entry.action.type === "trashPaths" ? "Moved to Trash from" : "Affected items"}
          </span>
          <ul className="flex flex-col divide-y divide-line">
            {entry.affectedPaths.map((path) => (
              <li key={path} className="flex items-center justify-between gap-3 py-1">
                <span className="num selectable min-w-0 break-all text-fg">{path}</span>
                <RevealButton path={path} />
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

function EntryRow({ entry, open, onToggle }: { entry: AuditEntry; open: boolean; onToggle: () => void }) {
  const reduced = usePrefersReducedMotion();
  const origin = originLabel(entry);
  const status = STATUS[entry.status];
  return (
    <li className="border-b border-line">
      <button
        type="button"
        onClick={onToggle}
        aria-expanded={open}
        className={cx(
          "grid h-[34px] w-full items-center gap-3 px-3 text-left text-[12px] transition-colors duration-150 hover:bg-raised",
          COLUMNS,
        )}
      >
        <span className="num text-fg-muted">{formatDateTime(entry.tsMs)}</span>
        <span className={cx("truncate", entry.origin.type === "agent" ? "text-fg" : "text-fg-muted")} title={origin.full}>
          {origin.short}
        </span>
        <span className="truncate text-fg" title={entry.title}>
          {entry.title}
        </span>
        <span className={status.className}>{status.label}</span>
        <span className="num text-right text-fg-muted">
          {entry.bytesFreed !== null && entry.bytesFreed > 0 ? formatBytes(entry.bytesFreed, { base: 1000 }) : ""}
        </span>
        <CaretRight
          size={11}
          weight="bold"
          className={cx("text-fg-subtle transition-transform duration-150", open && "rotate-90")}
        />
      </button>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            initial={reduced ? false : { opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={reduced ? undefined : { opacity: 0 }}
            transition={springInteraction}
          >
            <EntryDetails entry={entry} />
          </motion.div>
        )}
      </AnimatePresence>
    </li>
  );
}

export function AuditLogView() {
  const [filter, setFilter] = useState<OriginFilter>("any");
  /** Bumped to refetch the newest page (refresh button, retry, finished agent plans). */
  const [reload, setReload] = useState(0);
  const [entries, setEntries] = useState<AuditEntry[] | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [hasMore, setHasMore] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [openId, setOpenId] = useState<number | null>(null);

  useEffect(() => {
    let active = true;
    api.getAuditLog({ limit: PAGE_SIZE, beforeId: null, origin: filter }).then(
      (page) => {
        if (!active) return;
        setError(null);
        setEntries(page);
        setHasMore(page.length === PAGE_SIZE);
      },
      (e: unknown) => {
        if (active) setError(e);
      },
    );
    return () => {
      active = false;
    };
  }, [filter, reload]);

  // New entries appear when an agent plan finishes running.
  useEffect(() => {
    const unlisten = on("sentinel:agent", (payload) => {
      if (payload.event.type === "planUpdated" && payload.event.plan.status === "completed") setReload((n) => n + 1);
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  const refresh = () => setReload((n) => n + 1);

  const loadOlder = async () => {
    const last = entries?.[entries.length - 1];
    if (!last) return;
    setLoadingMore(true);
    try {
      const page = await api.getAuditLog({ limit: PAGE_SIZE, beforeId: last.id, origin: filter });
      setEntries((current) => [...(current ?? []), ...page]);
      setHasMore(page.length === PAGE_SIZE);
    } catch (e) {
      setError(e);
    } finally {
      setLoadingMore(false);
    }
  };

  return (
    <div className="mx-auto flex max-w-[1180px] flex-col px-6 py-5">
      <div className="flex flex-wrap items-end justify-between gap-4 pb-4">
        <div className="flex flex-col gap-1">
          <h1 className="text-[20px] font-semibold tracking-display text-fg">Activity</h1>
          <p className="text-[12px] text-fg-muted">
            Every change Sentinel made or declined, whether you started it or approved it in an agent plan, with measured results.
          </p>
        </div>
        <div className="flex items-center gap-1.5">
          <Segmented<OriginFilter>
            label="Show actions from"
            value={filter}
            onChange={(next) => {
              setEntries(null);
              setFilter(next);
            }}
            options={[
              { value: "any", label: "All" },
              { value: "user", label: "You" },
              { value: "agent", label: "Agent" },
            ]}
          />
          <IconButton label="Refresh activity" onClick={refresh}>
            <ArrowsClockwise size={14} />
          </IconButton>
        </div>
      </div>

      <div className={cx("grid gap-3 border-b border-line px-3 pb-1.5 text-[11px] text-fg-subtle", COLUMNS)}>
        <span>Time</span>
        <span>Started by</span>
        <span>Action</span>
        <span>Outcome</span>
        <span className="text-right">Freed</span>
        <span />
      </div>

      {error !== null && entries === null ? (
        <StatePanel>
          <ErrorState error={error} subject="The activity log" onRetry={refresh} />
        </StatePanel>
      ) : entries === null ? (
        <SkeletonRows rows={10} columns={[3, 3, 5, 2, 2]} rowHeight={34} />
      ) : entries.length === 0 ? (
        <StatePanel>
          <EmptyState
            title={filter === "agent" ? "No agent actions yet" : filter === "user" ? "No actions from you yet" : "No actions yet"}
            detail="When you confirm an action or run an agent plan, it is recorded here with its result. Declined proposals are recorded too."
          />
        </StatePanel>
      ) : (
        <>
          <ul>
            {entries.map((entry) => (
              <EntryRow
                key={entry.id}
                entry={entry}
                open={openId === entry.id}
                onToggle={() => setOpenId((current) => (current === entry.id ? null : entry.id))}
              />
            ))}
          </ul>
          {error !== null && <ErrorState error={error} subject="Loading older entries" compact className="mt-3" />}
          {hasMore && (
            <div className="flex justify-center py-4">
              <Button onClick={() => void loadOlder()} disabled={loadingMore}>
                {loadingMore ? "Loading older entries" : "Load older entries"}
              </Button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
