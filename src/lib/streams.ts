// Ref-counted registry of live sampler streams. Views declare what they need while visible; the
// union (plus the always-on `resources` stream) is pushed to the backend with `set_sampling`.
import type { SamplingConfig } from "../bindings/SamplingConfig";
import type { StreamKind } from "../bindings/StreamKind";

export type SamplingSender = (config: SamplingConfig) => Promise<SamplingConfig>;

const ORDER: StreamKind[] = ["resources", "processes", "network"];

export class StreamRegistry {
  private counts = new Map<StreamKind, number>();
  private intervalMs: number;
  private lastSent: string | null = null;
  private scheduled = false;
  private listeners = new Set<(config: SamplingConfig) => void>();
  private errorListeners = new Set<(error: unknown) => void>();

  constructor(
    private readonly send: SamplingSender,
    intervalMs = 1000,
    private readonly schedule: (fn: () => void) => void = (fn) => queueMicrotask(fn),
  ) {
    this.intervalMs = intervalMs;
  }

  /** Declare a need for `kind`; call the returned function to release it. Idempotent release. */
  acquire(kind: StreamKind): () => void {
    this.counts.set(kind, (this.counts.get(kind) ?? 0) + 1);
    this.flush();
    let released = false;
    return () => {
      if (released) return;
      released = true;
      const next = (this.counts.get(kind) ?? 1) - 1;
      if (next <= 0) this.counts.delete(kind);
      else this.counts.set(kind, next);
      this.flush();
    };
  }

  setInterval(intervalMs: number): void {
    this.intervalMs = intervalMs;
    this.flush();
  }

  streams(): StreamKind[] {
    return ORDER.filter((k) => k === "resources" || (this.counts.get(k) ?? 0) > 0);
  }

  config(): SamplingConfig {
    return { intervalMs: this.intervalMs, streams: this.streams() };
  }

  onApplied(listener: (config: SamplingConfig) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  onError(listener: (error: unknown) => void): () => void {
    this.errorListeners.add(listener);
    return () => this.errorListeners.delete(listener);
  }

  /** Forces the next flush to resend even if unchanged (e.g. after a backend restart). */
  invalidate(): void {
    this.lastSent = null;
    this.flush();
  }

  private flush(): void {
    if (this.scheduled) return;
    this.scheduled = true;
    this.schedule(() => {
      this.scheduled = false;
      const config = this.config();
      const key = JSON.stringify(config);
      if (key === this.lastSent) return;
      this.lastSent = key;
      this.send(config).then(
        (applied) => this.listeners.forEach((l) => l(applied)),
        (error: unknown) => {
          this.lastSent = null;
          this.errorListeners.forEach((l) => l(error));
        },
      );
    });
  }
}
