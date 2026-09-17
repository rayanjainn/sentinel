import { ArrowsClockwise, Info, LockKey, Warning } from "@phosphor-icons/react";
import { useState, type ReactNode } from "react";

import { describeError } from "../lib/errors";
import { cx } from "../lib/cx";
import { api } from "../lib/ipc";
import { Button } from "./Button";

const PERMISSION_LABEL = {
  fullDiskAccess: "Open Full Disk Access settings",
  administrator: "Open administrator settings",
} as const;

export function PermissionButton({ kind, size = "md" }: { kind: keyof typeof PERMISSION_LABEL; size?: "sm" | "md" }) {
  const [failed, setFailed] = useState<string | null>(null);
  return (
    <span className="inline-flex items-center gap-2">
      <Button
        size={size}
        icon={<LockKey size={14} weight="bold" />}
        onClick={() => {
          setFailed(null);
          api.openPermissionSettings(kind).catch((e: unknown) => setFailed(describeError(e).detail));
        }}
      >
        {PERMISSION_LABEL[kind]}
      </Button>
      {failed && <span className="text-[12px] text-fg-subtle">{failed}</span>}
    </span>
  );
}

/**
 * Specific error rendering for any failed backend call. `unavailable` renders as a quiet "not
 * available on this system" note; permission problems offer the relevant settings pane.
 */
export function ErrorState({
  error,
  subject,
  onRetry,
  compact = false,
  className,
}: {
  error: unknown;
  subject?: string;
  onRetry?: () => void;
  compact?: boolean;
  className?: string;
}) {
  const d = describeError(error, subject);
  const Icon = d.quiet ? Info : d.permission ? LockKey : Warning;
  return (
    <div
      role={d.quiet ? "note" : "alert"}
      className={cx("flex gap-3", compact ? "items-start" : "max-w-[46ch] items-start", className)}
    >
      <Icon
        size={compact ? 14 : 16}
        weight="bold"
        className={cx("mt-0.5 shrink-0", d.quiet ? "text-fg-subtle" : d.permission ? "text-warn" : "text-danger")}
      />
      <div className="flex min-w-0 flex-col gap-1">
        <p className={cx("font-medium", d.quiet ? "text-fg-muted" : "text-fg", compact && "text-[12px]")}>{d.title}</p>
        <p className={cx("selectable text-fg-muted", compact ? "text-[12px]" : "text-[13px]")}>{d.detail}</p>
        {(onRetry || d.permission) && (
          <div className="mt-2 flex flex-wrap items-center gap-2">
            {d.permission && <PermissionButton kind={d.permission} size={compact ? "sm" : "md"} />}
            {onRetry && !d.quiet && (
              <Button size={compact ? "sm" : "md"} variant="ghost" icon={<ArrowsClockwise size={14} />} onClick={onRetry}>
                Try again
              </Button>
            )}
            {onRetry && d.quiet && (
              <Button size="sm" variant="ghost" icon={<ArrowsClockwise size={13} />} onClick={onRetry}>
                Check again
              </Button>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

/** Centered state for an empty or failed region. */
export function StatePanel({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={cx("flex h-full min-h-40 w-full items-center justify-center p-8", className)}>{children}</div>;
}

export function EmptyState({
  icon,
  title,
  detail,
  action,
  className,
}: {
  icon?: ReactNode;
  title: string;
  detail?: ReactNode;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <div className={cx("flex max-w-[44ch] flex-col items-start gap-1.5", className)}>
      {icon && <div className="mb-1 text-fg-subtle">{icon}</div>}
      <p className="font-medium text-fg">{title}</p>
      {detail && <p className="text-fg-muted">{detail}</p>}
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}
