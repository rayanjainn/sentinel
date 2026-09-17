import { Stop, Warning } from "@phosphor-icons/react";

import type { ScanPhase } from "../../bindings/ScanPhase";
import { Button } from "../../components/Button";
import { PermissionButton } from "../../components/States";
import { cx } from "../../lib/cx";
import { formatBytes, formatCount, formatElapsedMs, middleTruncatePath, pluralize } from "../../lib/format";
import { usePermissions } from "../../stores/permissions";
import { isScanning, useStorage } from "../../stores/storage";

const PHASE_LABEL: Record<ScanPhase, string> = {
  walking: "Scanning",
  summarizing: "Summarizing",
  complete: "Scan complete",
  cancelled: "Scan cancelled",
  failed: "Scan failed",
};

/** Live scan progress: counts, current path, unreadable entries with a Full Disk Access hint. */
export function ScanBar() {
  const scanId = useStorage((s) => s.scanId);
  const progress = useStorage((s) => s.progress);
  const summary = useStorage((s) => s.summary);
  const cancelScan = useStorage((s) => s.cancelScan);
  const fda = usePermissions((s) => s.status?.fullDiskAccess);
  if (!scanId) return null;
  const scanning = isScanning(progress, scanId);
  const phase: ScanPhase = progress?.phase ?? "walking";
  const unreadable = summary?.unreadableEntries ?? progress?.unreadableEntries ?? 0;
  const needsFda = unreadable > 0 && fda !== "granted" && fda !== "notApplicable";

  return (
    <div className="relative shrink-0 border-b border-line px-5 py-2.5">
      {scanning && <div className="skeleton absolute inset-x-0 top-0 h-0.5 rounded-none" aria-hidden />}
      <div className="flex flex-wrap items-center gap-x-6 gap-y-1.5 text-[12px]">
        <span className={cx("font-medium", phase === "failed" ? "text-danger" : phase === "cancelled" ? "text-warn" : "text-fg")}>
          {PHASE_LABEL[phase]}
        </span>
        <span className="text-fg-muted">
          <span className="num text-fg">{formatCount(summary?.fileCount ?? progress?.filesScanned ?? 0)}</span> files
        </span>
        <span className="text-fg-muted">
          <span className="num text-fg">{formatCount(summary?.dirCount ?? progress?.dirsScanned ?? 0)}</span> folders
        </span>
        <span className="num text-fg">{formatBytes(summary?.totalBytes ?? progress?.bytesScanned ?? 0, { base: 1000 })}</span>
        <span className="num text-fg-muted">{formatElapsedMs(summary?.durationMs ?? progress?.elapsedMs ?? 0)}</span>
        {scanning && progress?.currentPath && (
          <span className="num min-w-0 flex-1 truncate text-fg-subtle" title={progress.currentPath}>
            {middleTruncatePath(progress.currentPath, 80)}
          </span>
        )}
        {!scanning && <span className="flex-1" />}
        {scanning && (
          <Button size="sm" variant="ghost" icon={<Stop size={12} weight="fill" />} onClick={() => void cancelScan()}>
            Cancel scan
          </Button>
        )}
      </div>
      {progress?.error && <p className="mt-1 text-[12px] text-danger">{progress.error}</p>}
      {unreadable > 0 && (
        <div className="mt-1.5 flex flex-wrap items-center gap-3 text-[12px] text-warn">
          <span className="flex items-center gap-1.5">
            <Warning size={13} weight="bold" />
            {pluralize(unreadable, "item")} could not be read, so some folders are smaller than they really are.
          </span>
          {needsFda && <PermissionButton kind="fullDiskAccess" size="sm" />}
          {summary && summary.unreadableSamples.length > 0 && (
            <span className="num truncate text-fg-subtle" title={summary.unreadableSamples.join("\n")}>
              {summary.unreadableSamples.slice(0, 2).join(", ")}
            </span>
          )}
        </div>
      )}
    </div>
  );
}
