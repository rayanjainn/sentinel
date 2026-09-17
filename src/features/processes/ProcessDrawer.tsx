import {
  ArrowsClockwise,
  CheckCircle,
  Copy,
  File,
  FolderOpen,
  FolderSimple,
  Gauge,
  HardDrive,
  Pipe,
  Plug,
  Power,
  Question,
  ShieldWarning,
  WarningOctagon,
  X,
  XCircle,
} from "@phosphor-icons/react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import type { OpenFile } from "../../bindings/OpenFile";
import type { ProcessDetail } from "../../bindings/ProcessDetail";
import type { ProcessHistoryPoint } from "../../bindings/ProcessHistoryPoint";
import type { ProcessIdentity } from "../../bindings/ProcessIdentity";
import type { ProcessInfo } from "../../bindings/ProcessInfo";
import type { QuitSafety } from "../../bindings/QuitSafety";
import { AnimatedNumber } from "../../components/AnimatedNumber";
import { Button, IconButton } from "../../components/Button";
import { LiveChart, type DataSource } from "../../components/charts/LiveChart";
import { InfoTip } from "../../components/InfoTip";
import { Segmented } from "../../components/Segmented";
import { Skeleton } from "../../components/Skeleton";
import { ErrorState } from "../../components/States";
import { cx } from "../../lib/cx";
import { formatBytes, formatDateTime, formatDuration, formatNice, formatPercent } from "../../lib/format";
import type { GlossaryId } from "../../lib/glossary";
import { api } from "../../lib/ipc";
import { springPanel } from "../../lib/motion";
import { formatEndpoint, TCP_STATE_LABEL } from "../../lib/net";
import { fileManagerName } from "../../lib/platform";
import { mergeSeries } from "../../lib/series";
import { useVirtualRows } from "../../lib/virtual";
import { useProcesses } from "../../stores/processes";
import { useSettings } from "../../stores/settings";
import { SERIES } from "../../styles/dataviz";
import { STATUS_LABEL } from "./columns";
import { openPriorityDialog } from "./PriorityDialog";
import { copyCommandLine, forceQuitProcess, quitProcess, revealExecutable } from "./processActions";
import { useProcessView } from "./viewState";

const HISTORY_WINDOW_MS = 60_000;

const SAFETY_TONE: Record<QuitSafety, { icon: typeof Question; className: string }> = {
  systemCritical: { icon: WarningOctagon, className: "text-danger" },
  osService: { icon: ShieldWarning, className: "text-warn" },
  appHelper: { icon: Question, className: "text-fg-muted" },
  userApp: { icon: CheckCircle, className: "text-signal" },
  background: { icon: Question, className: "text-fg-muted" },
  unknown: { icon: Question, className: "text-fg-subtle" },
};

/** The same sentence the confirm dialog shows for this process (PreviewTarget.safetyNote), so the
 * two can never disagree — see crates/sentinel-core/src/service/explain. */
function SafetyBlock({ detail }: { detail: ProcessDetail }) {
  const { explanation } = detail;
  const tone = SAFETY_TONE[explanation.quitSafety];
  const Icon = tone.icon;
  return (
    <div className="flex flex-col gap-2 border-b border-line px-5 py-3">
      <div className="flex items-start gap-2">
        <Icon size={15} weight="fill" className={cx("mt-0.5 shrink-0", tone.className)} />
        <div className="flex min-w-0 flex-col gap-1">
          <span className="text-[12px] font-medium text-fg">Is it safe to quit?</span>
          <p className="text-[13px] text-fg-muted">{explanation.quitNote}</p>
        </div>
      </div>
      {explanation.evidence.length > 0 && (
        <details className="pl-[23px] text-[12px] text-fg-subtle">
          <summary className="cursor-pointer select-none hover:text-fg">Evidence</summary>
          <ul className="num mt-1 flex flex-col gap-0.5 selectable">
            {explanation.evidence.map((line) => (
              <li key={line} className="break-all">
                {line}
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}

function useProcessHistory(identity: ProcessIdentity): { source: DataSource<ProcessHistoryPoint>; error: unknown } {
  const data = useRef<ProcessHistoryPoint[]>([]);
  const listeners = useRef(new Set<() => void>());
  const [error, setError] = useState<unknown>(null);

  const source = useMemo<DataSource<ProcessHistoryPoint>>(
    () => ({
      get: () => data.current,
      subscribe: (listener) => {
        listeners.current.add(listener);
        return () => listeners.current.delete(listener);
      },
    }),
    [],
  );

  useEffect(() => {
    const notify = () => listeners.current.forEach((l) => l());
    const push = (points: ProcessHistoryPoint[]) => {
      data.current = mergeSeries(data.current, points, HISTORY_WINDOW_MS + 10_000);
      notify();
    };
    let cancelled = false;
    api.getProcessHistory(identity.pid).then(
      (points) => !cancelled && push(points),
      (e: unknown) => !cancelled && setError(e),
    );
    const unsubscribe = useProcesses.subscribe((state, prev) => {
      if (state.tsMs === prev.tsMs) return;
      const p = state.byPid.get(identity.pid);
      if (p && p.startTime === identity.startTime) {
        push([{ tsMs: state.tsMs, cpuPercent: p.cpuPercent, memoryRss: p.memoryRss }]);
      }
    });
    return () => {
      cancelled = true;
      unsubscribe();
    };
  }, [identity.pid, identity.startTime]);

  return { source, error };
}

type DetailState = { status: "loading" } | { status: "ready"; detail: ProcessDetail } | { status: "error"; error: unknown };

function Meta({
  label,
  children,
  mono = false,
  glossaryId,
}: {
  label: string;
  children: ReactNode;
  mono?: boolean;
  glossaryId?: GlossaryId;
}) {
  return (
    <>
      <dt className="flex items-center gap-1 text-fg-muted">
        {label}
        {glossaryId && <InfoTip id={glossaryId} />}
      </dt>
      <dd className={cx("selectable min-w-0 break-words text-fg", mono && "num text-[12px]")}>{children}</dd>
    </>
  );
}

const FILE_ICON: Record<OpenFile["kind"], ReactNode> = {
  file: <File size={13} />,
  directory: <FolderSimple size={13} />,
  socket: <Plug size={13} />,
  pipe: <Pipe size={13} />,
  device: <HardDrive size={13} />,
  other: <Question size={13} />,
};

function OpenFilesList({ files }: { files: OpenFile[] }) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const { start, end, totalHeight } = useVirtualRows(scrollRef, files.length, 26, { overscan: 6 });
  if (files.length === 0) return <p className="py-3 text-fg-muted">No open files reported.</p>;
  return (
    <div ref={scrollRef} className="max-h-72 overflow-y-auto rounded-[6px] border border-line bg-sunken">
      <div className="relative" style={{ height: totalHeight }}>
        {files.slice(start, end).map((f, i) => (
          <div
            key={`${f.fd ?? "x"}-${start + i}`}
            className="absolute left-0 right-0 flex h-[26px] items-center gap-2 px-2.5 text-[12px]"
            style={{ transform: `translateY(${(start + i) * 26}px)` }}
          >
            <span className="num w-8 shrink-0 text-right text-fg-subtle">{f.fd ?? "—"}</span>
            <span className="shrink-0 text-fg-muted" title={f.kind}>
              {FILE_ICON[f.kind]}
            </span>
            <span className="num selectable truncate text-fg" title={f.path ?? undefined}>
              {f.path ?? <span className="text-fg-subtle">{f.kind}</span>}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

function ConnectionsList({ detail }: { detail: ProcessDetail }) {
  if (detail.connections.length === 0) return <p className="py-3 text-fg-muted">No open network connections.</p>;
  return (
    <ul className="max-h-72 divide-y divide-line overflow-y-auto rounded-[6px] border border-line bg-sunken">
      {detail.connections.map((c) => (
        <li key={c.id} className="flex flex-col gap-0.5 px-2.5 py-1.5 text-[12px]">
          <div className="flex items-center justify-between gap-3">
            <span className="num selectable truncate text-fg">
              {c.remoteAddr ? formatEndpoint(c.remoteAddr, c.remotePort, c.family) : formatEndpoint(c.localAddr, c.localPort, c.family)}
            </span>
            <span className="shrink-0 text-fg-muted">
              {c.protocol.toUpperCase()} {c.state ? TCP_STATE_LABEL[c.state] : ""}
            </span>
          </div>
          <span className="truncate text-fg-subtle">
            {c.remoteHost ?? (c.remoteAddr ? `from ${formatEndpoint(c.localAddr, c.localPort, c.family)}` : "Local listener")}
          </span>
        </li>
      ))}
    </ul>
  );
}

function DrawerContent({ identity }: { identity: ProcessIdentity }) {
  const live = useProcesses((s) => {
    const p = s.byPid.get(identity.pid);
    return p && p.startTime === identity.startTime ? p : null;
  });
  const byPid = useProcesses((s) => s.byPid);
  const intervalMs = useSettings((s) => s.intervalMs);
  const closeDrawer = useProcessView((s) => s.closeDrawer);
  const [detailState, setDetailState] = useState<DetailState>({ status: "loading" });
  const [tab, setTab] = useState<"files" | "connections">("files");
  const [refreshKey, setRefreshKey] = useState(0);
  const { source, error: historyError } = useProcessHistory(identity);

  useEffect(() => {
    let cancelled = false;
    api.getProcessDetail(identity.pid).then(
      (detail) => !cancelled && setDetailState({ status: "ready", detail }),
      (error: unknown) => !cancelled && setDetailState({ status: "error", error }),
    );
    return () => {
      cancelled = true;
    };
  }, [identity.pid, refreshKey]);

  const detail = detailState.status === "ready" ? detailState.detail : null;
  const p: ProcessInfo | null = live ?? detail?.info ?? null;
  const exited = !live && (detailState.status !== "loading" || useProcesses.getState().status === "ready");
  const parent = p?.ppid != null ? byPid.get(p.ppid) : undefined;

  const cpuSeries = useMemo(
    () => [{ key: "cpu", label: "CPU", color: SERIES.processCpu, value: (d: ProcessHistoryPoint) => d.cpuPercent }],
    [],
  );
  const memSeries = useMemo(
    () => [{ key: "mem", label: "Memory", color: SERIES.processMemory, value: (d: ProcessHistoryPoint) => d.memoryRss }],
    [],
  );

  if (!p) {
    return (
      <div className="flex flex-col gap-4 p-5">
        <div className="flex items-start justify-between">
          <Skeleton className="h-4 w-40" />
          <IconButton label="Close details" size="sm" onClick={closeDrawer}>
            <X size={14} />
          </IconButton>
        </div>
        {detailState.status === "error" ? (
          <ErrorState error={detailState.error} subject="Process details" onRetry={() => setRefreshKey((k) => k + 1)} />
        ) : (
          <>
            <Skeleton className="h-3 w-24" />
            <Skeleton className="h-20 w-full" />
            <Skeleton className="h-20 w-full" />
          </>
        )}
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-start justify-between gap-3 border-b border-line px-5 pb-3 pt-4">
        <div className="flex min-w-0 flex-col gap-1">
          <h2 className="selectable truncate text-[15px] font-semibold tracking-display text-fg">{p.name}</h2>
          <div className="flex items-center gap-3 text-[12px] text-fg-muted">
            <span>
              PID <span className="num text-fg">{p.pid}</span>
            </span>
            <span>{p.user ?? "Unknown user"}</span>
            <span className={cx(exited ? "text-danger" : p.status === "running" ? "text-signal" : "text-fg-muted")}>
              {exited ? "No longer running" : STATUS_LABEL[p.status]}
            </span>
          </div>
        </div>
        <IconButton label="Close details" size="sm" onClick={closeDrawer} className="-mr-1.5">
          <X size={14} />
        </IconButton>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {detail && !exited && <SafetyBlock detail={detail} />}
        <div className="flex flex-wrap items-center gap-1.5 border-b border-line px-5 py-3">
          <Button size="sm" icon={<Power size={13} />} disabled={exited} onClick={() => void quitProcess(p)}>
            Quit
          </Button>
          <Button size="sm" variant="dangerQuiet" icon={<XCircle size={13} />} disabled={exited} onClick={() => void forceQuitProcess(p)}>
            Force quit
          </Button>
          <Button size="sm" variant="ghost" icon={<Gauge size={13} />} disabled={exited} onClick={() => openPriorityDialog(p)}>
            Priority
          </Button>
          <div className="flex-1" />
          <IconButton label={`Reveal in ${fileManagerName}`} size="sm" disabled={!p.exe} onClick={() => void revealExecutable(p)}>
            <FolderOpen size={14} />
          </IconButton>
          <IconButton label="Copy command line" size="sm" onClick={() => void copyCommandLine(p)}>
            <Copy size={14} />
          </IconButton>
        </div>
        {detail && !exited && (
          <p className="px-5 pt-2 text-[12px] text-fg-subtle">
            {detail.hasWindow ? "Quit asks the app to close its windows first." : "Quit sends a termination request the process can handle."}
          </p>
        )}

        <div className="grid grid-cols-2 gap-5 px-5 py-4">
          <div className="flex min-w-0 flex-col gap-1.5">
            <div className="flex items-baseline justify-between">
              <span className="text-[12px] text-fg-muted">CPU, last 60 s</span>
              <AnimatedNumber value={live?.cpuPercent ?? 0} format={formatPercent} className="num text-[15px] text-fg" />
            </div>
            <LiveChart
              source={source}
              series={cpuSeries}
              time={(d) => d.tsMs}
              windowMs={HISTORY_WINDOW_MS}
              intervalMs={intervalMs}
              height={64}
              yFloor={10}
              grid={[1]}
              formatValue={formatPercent}
              ariaLabel={`CPU usage of ${p.name} over the last minute`}
            />
          </div>
          <div className="flex min-w-0 flex-col gap-1.5">
            <div className="flex items-baseline justify-between">
              <span className="text-[12px] text-fg-muted">Memory, last 60 s</span>
              <AnimatedNumber value={live?.memoryRss ?? p.memoryRss} format={formatBytes} className="num text-[15px] text-fg" />
            </div>
            <LiveChart
              source={source}
              series={memSeries}
              time={(d) => d.tsMs}
              windowMs={HISTORY_WINDOW_MS}
              intervalMs={intervalMs}
              height={64}
              yFloor={64 * 1024 * 1024}
              grid={[1]}
              formatValue={formatBytes}
              ariaLabel={`Memory usage of ${p.name} over the last minute`}
            />
          </div>
          {historyError !== null && (
            <ErrorState error={historyError} subject="Process history" compact className="col-span-2" />
          )}
        </div>

        <dl className="grid grid-cols-[128px_minmax(0,1fr)] gap-x-4 gap-y-2 border-t border-line px-5 py-4 text-[13px]">
          <Meta label="Parent" glossaryId="ppid">
            {p.ppid === null ? (
              "—"
            ) : parent ? (
              <button
                type="button"
                className="text-left hover:text-signal"
                onClick={() => useProcessView.getState().select({ pid: parent.pid, startTime: parent.startTime }, true)}
              >
                {parent.name} <span className="num text-fg-muted">({parent.pid})</span>
              </button>
            ) : (
              <span className="num">{p.ppid}</span>
            )}
          </Meta>
          <Meta label="Average CPU" mono glossaryId="cpuPercentAvg">{formatPercent(p.cpuPercentAvg)}</Meta>
          <Meta label="Virtual memory" mono glossaryId="memoryVirtual">{formatBytes(p.memoryVirtual)}</Meta>
          <Meta label="Threads" mono glossaryId="threadCount">{p.threadCount ?? "Not available"}</Meta>
          <Meta label="Open files" mono glossaryId="fdCount">{p.fdCount ?? "Not available"}</Meta>
          <Meta label="Nice" mono glossaryId="nice">{formatNice(p.nice)}</Meta>
          <Meta label="Started" glossaryId="startTime">{formatDateTime(p.startTime * 1000)}</Meta>
          <Meta label="Running for" mono glossaryId="runTime">{formatDuration(p.runTimeSecs)}</Meta>
          <Meta label="Executable" mono>{p.exe ?? "Not available"}</Meta>
          <Meta label="Command line" mono>
            <span className="block max-h-28 overflow-y-auto whitespace-pre-wrap break-all">{p.cmd.length > 0 ? p.cmd.join(" ") : "Not available"}</span>
          </Meta>
        </dl>

        <div className="flex flex-col gap-3 border-t border-line px-5 py-4">
          <div className="flex items-center justify-between">
            <Segmented
              label="Handles"
              size="sm"
              value={tab}
              onChange={setTab}
              options={[
                { value: "files", label: `Open files${detail ? ` ${detail.openFiles.length}` : ""}` },
                { value: "connections", label: `Connections${detail ? ` ${detail.connections.length}` : ""}` },
              ]}
            />
            <IconButton label="Refresh handles" size="sm" onClick={() => setRefreshKey((k) => k + 1)}>
              <ArrowsClockwise size={13} />
            </IconButton>
          </div>
          {detailState.status === "loading" && (
            <div className="flex flex-col gap-2">
              {Array.from({ length: 5 }, (_, i) => (
                <Skeleton key={i} className="h-3" style={{ width: `${90 - i * 9}%` }} />
              ))}
            </div>
          )}
          {detailState.status === "error" && (
            <ErrorState error={detailState.error} subject="Open files and connections" compact onRetry={() => setRefreshKey((k) => k + 1)} />
          )}
          {detail && tab === "files" && (
            <>
              {detail.openFilesError && (
                <ErrorState error={detail.openFilesError} subject="Open files" compact />
              )}
              {!detail.openFilesError && <OpenFilesList files={detail.openFiles} />}
            </>
          )}
          {detail && tab === "connections" && <ConnectionsList detail={detail} />}
        </div>
      </div>
    </div>
  );
}

export function ProcessDrawer() {
  const selected = useProcessView((s) => s.selected);
  const open = useProcessView((s) => s.drawerOpen);
  return (
    <AnimatePresence>
      {open && selected && (
        <motion.aside
          key="process-drawer"
          aria-label="Process details"
          initial={{ x: "100%" }}
          animate={{ x: 0 }}
          exit={{ x: "100%" }}
          transition={springPanel}
          className="shadow-float absolute inset-y-0 right-0 z-20 w-[460px] max-w-[70%] rounded-l-[12px] bg-panel"
        >
          <DrawerContent key={`${selected.pid}:${selected.startTime}`} identity={selected} />
        </motion.aside>
      )}
    </AnimatePresence>
  );
}
