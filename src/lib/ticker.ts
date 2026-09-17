// One shared requestAnimationFrame loop for every continuously scrolling graph, so twenty per-core
// sparklines cost one frame callback instead of twenty.
type FrameCallback = (nowMs: number) => void;

const callbacks = new Set<FrameCallback>();
let handle = 0;

function loop() {
  handle = 0;
  const now = Date.now();
  callbacks.forEach((cb) => cb(now));
  if (callbacks.size > 0) handle = requestAnimationFrame(loop);
}

export function onFrame(cb: FrameCallback): () => void {
  callbacks.add(cb);
  if (!handle) handle = requestAnimationFrame(loop);
  return () => {
    callbacks.delete(cb);
    if (callbacks.size === 0 && handle) {
      cancelAnimationFrame(handle);
      handle = 0;
    }
  };
}
