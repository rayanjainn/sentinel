import { useEffect, useState } from "react";

import { describeError } from "../lib/errors";
import { cx } from "../lib/cx";
import { useSamplingStatus } from "../lib/sampling";
import { useResources } from "../stores/resources";
import { useSettings } from "../stores/settings";

type Live = "live" | "waiting" | "off";

/** Sampling heartbeat: pulses only while samples are actually arriving. */
export function LiveIndicator() {
  const intervalMs = useSettings((s) => s.intervalMs);
  const lastEventAt = useResources((s) => s.lastEventAt);
  const samplingError = useSamplingStatus((s) => s.error);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  const fresh = lastEventAt !== null && now - lastEventAt < intervalMs * 3 + 1000;
  const state: Live = fresh ? "live" : samplingError ? "off" : "waiting";
  const label =
    state === "live"
      ? `Live, every ${intervalMs >= 1000 ? `${intervalMs / 1000} s` : `${intervalMs} ms`}`
      : state === "waiting"
        ? "Waiting for samples"
        : "Not sampling";
  const title = state === "off" ? describeError(samplingError).detail : label;

  return (
    <div className="flex items-center gap-2 text-[12px] text-fg-muted" title={title} aria-live="polite">
      <span className="relative flex size-2 items-center justify-center">
        {state === "live" && <span className="live-ping absolute inset-0 rounded-full bg-signal" />}
        <span
          className={cx(
            "relative size-1.5 rounded-full",
            state === "live" && "bg-signal",
            state === "waiting" && "bg-warn",
            state === "off" && "bg-fg-subtle",
          )}
        />
      </span>
      <span>{label}</span>
    </div>
  );
}
