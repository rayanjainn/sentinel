import { motion } from "motion/react";

import { Button } from "../../components/Button";
import { springPanel } from "../../lib/motion";
import { isMac } from "../../lib/platform";
import { usePermissions } from "../../stores/permissions";
import { useSettings } from "../../stores/settings";
import { PermissionList } from "../permissions/PermissionList";

/** First-run screen explaining optional elevated access. Monitoring works without either. */
export function Onboarding() {
  const complete = useSettings((s) => s.completeOnboarding);
  const status = usePermissions((s) => s.status);
  const fdaDone = status ? status.fullDiskAccess === "granted" || status.fullDiskAccess === "notApplicable" : false;
  const adminDone = status ? status.runningElevated || status.canRequestElevation : false;
  const allSet = fdaDone && adminDone;

  return (
    <div className="flex h-full flex-col bg-ground">
      <div data-tauri-drag-region className={isMac ? "h-[52px] shrink-0" : "h-6 shrink-0"} />
      <div className="flex min-h-0 flex-1 items-center overflow-y-auto px-10 pb-12">
        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={springPanel}
          className="mx-auto grid w-full max-w-[1040px] grid-cols-[minmax(0,5fr)_minmax(0,6fr)] items-start gap-16"
        >
          <div className="flex flex-col gap-5 pt-2">
            <div className="flex items-center gap-2.5">
              <svg width="22" height="22" viewBox="0 0 16 16" aria-hidden className="text-signal">
                <circle cx="8" cy="8" r="6.5" fill="none" stroke="currentColor" strokeOpacity="0.35" strokeWidth="1" />
                <circle cx="8" cy="8" r="3.5" fill="none" stroke="currentColor" strokeOpacity="0.7" strokeWidth="1" />
                <circle cx="8" cy="8" r="1.4" fill="currentColor" />
              </svg>
              <span className="text-[15px] font-semibold tracking-display">Sentinel</span>
            </div>
            <h2 className="text-[28px] font-semibold leading-tight tracking-display text-fg">
              See everything this computer is doing, and act on it.
            </h2>
            <p className="max-w-[46ch] text-[13px] leading-relaxed text-fg-muted">
              Sentinel reads processes, network connections, CPU, memory and storage directly from the operating
              system. Nothing is sent anywhere unless you connect a cloud AI provider yourself.
            </p>
            <p className="max-w-[46ch] text-[13px] leading-relaxed text-fg-muted">
              Two kinds of access make the picture complete. Both are optional: live monitoring works without them,
              and you can revisit this any time in Settings.
            </p>
            <div className="mt-3 flex items-center gap-3">
              {allSet ? (
                <Button variant="primary" onClick={complete}>
                  Continue
                </Button>
              ) : (
                <Button variant="primary" onClick={complete}>
                  Continue with limited access
                </Button>
              )}
              {!allSet && <span className="text-[12px] text-fg-subtle">Status updates when you return from Settings.</span>}
            </div>
          </div>
          <section aria-label="Permissions" className="rounded-[8px] border border-line bg-panel p-6">
            <PermissionList />
          </section>
        </motion.div>
      </div>
    </div>
  );
}
