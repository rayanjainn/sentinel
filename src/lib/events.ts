// Synchronous-dispose wrapper over `on()` so effects can subscribe and clean up without races.
import { on, type EventMap } from "./ipc";

export function subscribe<K extends keyof EventMap>(
  event: K,
  handler: (payload: EventMap[K]) => void,
): () => void {
  let disposed = false;
  let unlisten: (() => void) | null = null;
  on(event, (payload) => {
    if (!disposed) handler(payload);
  }).then(
    (fn) => {
      if (disposed) fn();
      else unlisten = fn;
    },
    // Outside the Tauri webview there is no event bus; commands surface that as errors instead.
    () => undefined,
  );
  return () => {
    disposed = true;
    unlisten?.();
    unlisten = null;
  };
}
