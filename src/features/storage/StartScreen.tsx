import { HardDrive, House } from "@phosphor-icons/react";

import { Button } from "../../components/Button";
import { Skeleton } from "../../components/Skeleton";
import { ErrorState } from "../../components/States";
import { formatBytes, formatPercent } from "../../lib/format";
import { useStorage } from "../../stores/storage";

export function StartScreen({ onScan }: { onScan: (root: string, name: string) => void }) {
  const volumes = useStorage((s) => s.volumes);
  const status = useStorage((s) => s.volumesStatus);
  const error = useStorage((s) => s.volumesError);
  const home = useStorage((s) => s.home);
  const loadVolumes = useStorage((s) => s.loadVolumes);
  const startError = useStorage((s) => s.startError);
  const system = volumes.find((v) => v.isSystem);

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto grid max-w-[1040px] grid-cols-[minmax(0,5fr)_minmax(0,6fr)] gap-14 px-8 py-12">
        <div className="flex flex-col gap-4">
          <h2 className="text-[20px] font-semibold tracking-display text-fg">See what is using your disk</h2>
          <p className="max-w-[46ch] text-fg-muted">
            A scan measures every folder and file, then draws them as a map you can zoom into. Results stream in while
            it runs. Nothing is deleted without your confirmation, and removals always go to the Trash.
          </p>
          {startError !== null && <ErrorState error={startError} subject="Starting the scan" compact />}
          <div className="mt-2 flex flex-wrap gap-2">
            <Button variant="primary" icon={<House size={14} />} disabled={!home} onClick={() => home && onScan(home, "Home")}>
              Scan home folder
            </Button>
            {system && (
              <Button icon={<HardDrive size={14} />} onClick={() => onScan(system.mountPoint, system.name || system.mountPoint)}>
                Scan {system.name || "entire disk"}
              </Button>
            )}
          </div>
          {home && <span className="num text-[12px] text-fg-subtle">{home}</span>}
        </div>

        <section aria-label="Volumes" className="flex flex-col gap-1">
          <h3 className="mb-2 text-[13px] font-medium text-fg">Volumes</h3>
          {status === "loading" &&
            Array.from({ length: 2 }, (_, i) => (
              <div key={i} className="flex flex-col gap-2 border-t border-line py-3">
                <Skeleton className="h-3 w-32" />
                <Skeleton className="h-1.5 w-full" />
              </div>
            ))}
          {status === "error" && <ErrorState error={error} subject="The volume list" compact onRetry={() => void loadVolumes()} />}
          {volumes.map((v) => {
            const used = v.totalBytes - v.availableBytes;
            const ratio = v.totalBytes > 0 ? used / v.totalBytes : 0;
            return (
              <button
                key={v.mountPoint}
                type="button"
                onClick={() => onScan(v.mountPoint, v.name || v.mountPoint)}
                className="group flex flex-col gap-2 rounded-[6px] border-t border-line px-2 py-3 text-left hover:bg-raised/60"
              >
                <div className="flex items-baseline justify-between gap-4">
                  <span className="flex items-center gap-2 text-fg">
                    <HardDrive size={14} className="text-fg-muted" />
                    {v.name || v.mountPoint}
                    {v.isRemovable && <span className="text-[11px] text-fg-subtle">Removable</span>}
                  </span>
                  <span className="text-[12px] text-fg-muted">
                    <span className="num text-fg">{formatBytes(v.availableBytes, { base: 1000 })}</span> free of{" "}
                    <span className="num">{formatBytes(v.totalBytes, { base: 1000 })}</span>
                  </span>
                </div>
                <div className="relative h-1.5 overflow-hidden rounded-full bg-[var(--viz-track)]">
                  <div
                    className={ratio > 0.9 ? "absolute inset-0 origin-left rounded-full bg-warn" : "absolute inset-0 origin-left rounded-full bg-[var(--viz-1)]"}
                    style={{ transform: `scaleX(${ratio})` }}
                  />
                </div>
                <div className="flex justify-between text-[11px] text-fg-subtle">
                  <span className="num">
                    {v.mountPoint} {v.fileSystem}
                  </span>
                  <span>
                    {formatPercent(ratio * 100, 0)} used <span className="opacity-0 transition-opacity group-hover:opacity-100">, click to scan</span>
                  </span>
                </div>
              </button>
            );
          })}
        </section>
      </div>
    </div>
  );
}
