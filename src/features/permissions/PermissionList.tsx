import { CheckCircle, Circle, MinusCircle, WarningCircle } from "@phosphor-icons/react";
import type { ReactNode } from "react";

import type { PermissionStatus } from "../../bindings/PermissionStatus";
import { Skeleton } from "../../components/Skeleton";
import { ErrorState, PermissionButton } from "../../components/States";
import { cx } from "../../lib/cx";
import { usePermissions } from "../../stores/permissions";

type Tone = "granted" | "attention" | "neutral" | "na";

function Badge({ tone, children }: { tone: Tone; children: ReactNode }) {
  const Icon = tone === "granted" ? CheckCircle : tone === "attention" ? WarningCircle : tone === "na" ? MinusCircle : Circle;
  return (
    <span
      className={cx(
        "inline-flex items-center gap-1.5 text-[12px] font-medium",
        tone === "granted" && "text-signal",
        tone === "attention" && "text-warn",
        (tone === "neutral" || tone === "na") && "text-fg-muted",
      )}
    >
      <Icon size={14} weight="fill" />
      {children}
    </span>
  );
}

function fullDiskAccess(status: PermissionStatus | null): { tone: Tone; label: string; actionable: boolean } {
  if (!status) return { tone: "neutral", label: "Status unavailable", actionable: true };
  switch (status.fullDiskAccess) {
    case "granted":
      return { tone: "granted", label: "Granted", actionable: false };
    case "denied":
      return { tone: "attention", label: "Not granted", actionable: true };
    case "unknown":
      return { tone: "neutral", label: "Could not be determined", actionable: true };
    case "notApplicable":
      return { tone: "na", label: "Not needed on this system", actionable: false };
  }
}

function administrator(status: PermissionStatus | null): { tone: Tone; label: string; actionable: boolean } {
  if (!status) return { tone: "neutral", label: "Status unavailable", actionable: true };
  if (status.runningElevated) return { tone: "granted", label: "Running with administrator rights", actionable: false };
  if (status.canRequestElevation) return { tone: "granted", label: "Requested when an action needs it", actionable: false };
  return { tone: "attention", label: "No way to request elevation found", actionable: true };
}

function Row({
  title,
  body,
  badge,
  action,
}: {
  title: string;
  body: ReactNode;
  badge: ReactNode;
  action: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-2 py-4 first:pt-0 last:pb-0">
      <div className="flex items-center justify-between gap-4">
        <h3 className="font-medium text-fg">{title}</h3>
        {badge}
      </div>
      <div className="max-w-[58ch] text-fg-muted">{body}</div>
      {action && <div className="mt-1">{action}</div>}
    </div>
  );
}

/** Live permission status with plain-language reasons and deep links to system settings. */
export function PermissionList({ className }: { className?: string }) {
  const { status, error, loading, refresh } = usePermissions();

  if (loading && !status && !error) {
    return (
      <div className={cx("flex flex-col gap-6", className)}>
        {[0, 1].map((i) => (
          <div key={i} className="flex flex-col gap-2">
            <Skeleton className="h-3 w-40" />
            <Skeleton className="h-2.5 w-full" />
            <Skeleton className="h-2.5 w-3/4" />
          </div>
        ))}
      </div>
    );
  }

  const fda = fullDiskAccess(status);
  const admin = administrator(status);
  return (
    <div className={cx("flex flex-col", className)}>
      {error !== null && !status && (
        <ErrorState error={error} subject="Permission status" compact onRetry={() => void refresh()} className="mb-4" />
      )}
      <div className="flex flex-col divide-y divide-line">
        <Row
          title="Full Disk Access"
          badge={<Badge tone={fda.tone}>{fda.label}</Badge>}
          body={
            <p>
              Lets storage scans read protected folders such as Mail, Messages and other apps’ data, so folder sizes
              are accurate instead of undercounted. Without it those folders are reported as unreadable.
            </p>
          }
          action={fda.actionable ? <PermissionButton kind="fullDiskAccess" /> : null}
        />
        <Row
          title="Administrator rights"
          badge={<Badge tone={admin.tone}>{admin.label}</Badge>}
          body={
            <p>
              Needed to add firewall rules, see details of processes owned by other users, and raise a process’s
              priority. Sentinel asks for authorization at the moment an action needs it, never in the background.
            </p>
          }
          action={admin.actionable ? <PermissionButton kind="administrator" /> : null}
        />
        {status?.firewall.note && (
          <p className="py-3 text-[12px] text-fg-subtle">{status.firewall.note}</p>
        )}
      </div>
    </div>
  );
}
