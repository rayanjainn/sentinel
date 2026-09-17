// Relative resource intensity: a process is tinted when it stands out from the *current*
// distribution (above the 95th percentile), scaled toward the busiest process. A small noise floor
// keeps an idle machine from tinting processes that merely rank first among near-zero values.
import type { ProcessInfo } from "../../bindings/ProcessInfo";

export function quantile(sorted: number[], q: number): number {
  if (sorted.length === 0) return 0;
  const pos = (sorted.length - 1) * q;
  const lo = Math.floor(pos);
  const hi = Math.ceil(pos);
  const a = sorted[lo]!;
  const b = sorted[hi]!;
  return a + (b - a) * (pos - lo);
}

/** Returns a scorer mapping a value to 0 (unremarkable) or (0.2, 1] (stands out). */
export function relativeScale(values: number[], noiseFloor: number, q = 0.95): (value: number) => number {
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  const top = sorted[sorted.length - 1] ?? 0;
  const threshold = Math.max(noiseFloor, quantile(sorted, q));
  if (top <= threshold) return () => 0;
  const span = top - threshold;
  return (value) => {
    if (!(value > threshold)) return 0;
    const t = Math.min(1, (value - threshold) / span);
    return 0.2 + 0.8 * Math.pow(t, 0.7);
  };
}

export interface IntensityModel {
  score: (p: ProcessInfo) => number;
  cpu: (p: ProcessInfo) => number;
  memory: (p: ProcessInfo) => number;
}

export function intensityModel(processes: ProcessInfo[], totalMemory: number): IntensityModel {
  // Floors: 2% of one core, 0.5% of physical memory.
  const cpuScale = relativeScale(
    processes.map((p) => p.cpuPercent),
    2,
  );
  const memScale = relativeScale(
    processes.map((p) => p.memoryRss),
    totalMemory > 0 ? totalMemory * 0.005 : 0,
  );
  const cpu = (p: ProcessInfo) => cpuScale(p.cpuPercent);
  const memory = (p: ProcessInfo) => memScale(p.memoryRss);
  return { cpu, memory, score: (p) => Math.max(cpu(p), memory(p)) };
}
