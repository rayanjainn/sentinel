import { Download, ShieldCheck } from "@phosphor-icons/react";

import type { GeoDbStatus } from "../../bindings/GeoDbStatus";
import { Button } from "../../components/Button";
import { ErrorState } from "../../components/States";
import { formatBytes } from "../../lib/format";
import { useNetwork } from "../../stores/network";

/** Explains and drives the one-time local geolocation database download. */
export function GeoDbNotice({ status, compact = false }: { status: GeoDbStatus; compact?: boolean }) {
  const downloadGeo = useNetwork((s) => s.downloadGeo);
  const downloadError = useNetwork((s) => s.downloadError);

  if (status.state === "ready") return null;

  if (status.state === "downloading") {
    const ratio = status.totalBytes ? Math.min(1, status.downloadedBytes / status.totalBytes) : null;
    return (
      <div className="flex w-full max-w-[380px] flex-col gap-2" role="status" aria-live="polite">
        <span className="font-medium text-fg">Downloading the location database</span>
        <div className="relative h-1 overflow-hidden rounded-full bg-[var(--viz-track)]">
          {ratio !== null ? (
            <div
              className="absolute inset-0 origin-left rounded-full bg-signal transition-transform duration-300 ease-out"
              style={{ transform: `scaleX(${ratio})` }}
            />
          ) : (
            <div className="skeleton absolute inset-0 rounded-full" />
          )}
        </div>
        <span className="num text-[12px] text-fg-muted">
          {formatBytes(status.downloadedBytes, { base: 1000 })}
          {status.totalBytes ? ` of ${formatBytes(status.totalBytes, { base: 1000 })}` : ""}
        </span>
      </div>
    );
  }

  return (
    <div className="flex max-w-[420px] flex-col gap-3">
      {!compact && <h3 className="text-[15px] font-semibold tracking-display text-fg">See where connections go</h3>}
      <p className="text-fg-muted">
        Sentinel places remote addresses on the map with a local IP-location database (DB-IP Lite). It downloads once;
        after that every lookup happens on this computer.
      </p>
      <p className="flex items-start gap-2 text-[12px] text-fg-subtle">
        <ShieldCheck size={14} className="mt-px shrink-0" />
        No address is ever sent to an online lookup service.
      </p>
      {status.state === "failed" && (
        <p className="text-[12px] text-danger">The last download failed: {status.message}</p>
      )}
      {downloadError !== null && status.state !== "failed" && <ErrorState error={downloadError} subject="The download" compact />}
      <div>
        <Button variant="primary" icon={<Download size={14} weight="bold" />} onClick={() => void downloadGeo()}>
          {status.state === "failed" ? "Try the download again" : "Download location database"}
        </Button>
      </div>
    </div>
  );
}
