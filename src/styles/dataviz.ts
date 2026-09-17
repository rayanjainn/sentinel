// Data-visualization palettes (docs/DESIGN.md "Color"). Data colors only — never chrome.
//
// Categorical slots follow the dataviz skill's reference order (blue, orange, aqua, yellow, magenta,
// green, violet). Validated with `validate_palette.js` against Sentinel's own surfaces:
//   dark  on #131820 and #0E1217: band, chroma, adjacent CVD (worst 8.4) and normal-vision (19.3)
//         pass; every slot >= 3:1.
//   light on #FFFFFF: band, chroma, CVD (worst 9.1), normal-vision (19.6) pass; aqua, yellow and
//         magenta sit below 3:1, so every use ships a relief channel (direct labels, tooltips and a
//         table view: treemap legend + by-type table, graph legends with values).
// Slot 8 (red) is deliberately unused so no series can be mistaken for `--danger`.
// Treemaps place any two kinds side by side (the all-pairs case), which seven hues cannot clear on
// their own; the treemap therefore always carries the 2px surface gap, in-cell labels where they
// fit, hover detail, and the by-type table as secondary encoding.
import type { FileKindGroup } from "../bindings/FileKindGroup";

type Pair = { dark: string; light: string };

export const categorical: Pair[] = [
  { dark: "#3987e5", light: "#2a78d6" }, // 1 blue
  { dark: "#d95926", light: "#eb6834" }, // 2 orange
  { dark: "#199e70", light: "#1baf7a" }, // 3 aqua
  { dark: "#c98500", light: "#eda100" }, // 4 yellow
  { dark: "#d55181", light: "#e87ba4" }, // 5 magenta
  { dark: "#008300", light: "#008300" }, // 6 green
  { dark: "#9085e9", light: "#4a3aa7" }, // 7 violet
];

/** Neutral steps for folded categories ("other", system files) and structural fills. */
const neutral = {
  folded: { dark: "#4b5563", light: "#9aa3ad" },
  foldedAlt: { dark: "#3a424d", light: "#b9c0c7" },
  directory: { dark: "#1f2630", light: "#dde3e9" },
  hatch: { dark: "#5d6774", light: "#87919d" },
};

export interface FileKindStyle {
  label: string;
  /** CSS custom property holding the fill. */
  varName: string;
  slot: number | null;
}

// Order is the legend order; slots are assigned in sequence and never cycled.
export const FILE_KIND_ORDER: FileKindGroup[] = [
  "document",
  "archive",
  "code",
  "audio",
  "image",
  "application",
  "video",
  "diskImage",
  "data",
  "system",
  "other",
];

export const FILE_KINDS: Record<FileKindGroup, FileKindStyle> = {
  document: { label: "Documents", varName: "--viz-kind-document", slot: 0 },
  archive: { label: "Archives", varName: "--viz-kind-archive", slot: 1 },
  code: { label: "Code", varName: "--viz-kind-code", slot: 2 },
  audio: { label: "Audio", varName: "--viz-kind-audio", slot: 3 },
  image: { label: "Images", varName: "--viz-kind-image", slot: 4 },
  application: { label: "Applications", varName: "--viz-kind-application", slot: 5 },
  video: { label: "Video", varName: "--viz-kind-video", slot: 6 },
  // Folded: disk images share the archive hue (both are packaged containers).
  diskImage: { label: "Disk images", varName: "--viz-kind-archive", slot: 1 },
  data: { label: "Data", varName: "--viz-kind-folded", slot: null },
  system: { label: "System", varName: "--viz-kind-folded-alt", slot: null },
  other: { label: "Other", varName: "--viz-kind-folded-alt", slot: null },
};

export function fileKindHex(kind: FileKindGroup | null, mode: "dark" | "light"): string {
  const style = kind ? FILE_KINDS[kind] : FILE_KINDS.other;
  if (style.slot !== null) return categorical[style.slot]![mode];
  return style.varName === "--viz-kind-folded" ? neutral.folded[mode] : neutral.foldedAlt[mode];
}

function luminance(hex: string): number {
  const n = parseInt(hex.slice(1), 16);
  const channel = (v: number) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel((n >> 16) & 255) + 0.7152 * channel((n >> 8) & 255) + 0.0722 * channel(n & 255);
}

/** Label ink for text set inside a filled mark: whichever of near-white or near-black contrasts more. */
export function inkOn(hex: string): string {
  const l = luminance(hex);
  const onDark = (1.05) / (l + 0.05);
  const onLight = (l + 0.05) / (luminance("#0e1217") + 0.05);
  return onDark >= onLight ? "#ffffff" : "#0e1217";
}

export function fileKindFill(kind: FileKindGroup | null): string {
  return `var(${kind ? FILE_KINDS[kind].varName : "--viz-kind-folded-alt"})`;
}

/** Series roles used by graphs. Each chart uses slots in order starting at 1. */
export const SERIES = {
  cpu: "var(--viz-1)",
  rx: "var(--viz-1)",
  tx: "var(--viz-2)",
  memory: "var(--viz-1)",
  processCpu: "var(--viz-1)",
  processMemory: "var(--viz-2)",
} as const;

/** Memory stack, bottom to top. `free` is the neutral remainder track. */
export const MEMORY_SERIES = {
  app: { label: "App memory", fill: "var(--viz-1)" },
  wired: { label: "Wired", fill: "var(--viz-2)" },
  compressed: { label: "Compressed", fill: "var(--viz-3)" },
  cached: { label: "Cached files", fill: "var(--viz-4)" },
  free: { label: "Free", fill: "var(--viz-track)" },
} as const;

function block(mode: "dark" | "light"): string {
  const lines = categorical.map((c, i) => `--viz-${i + 1}: ${c[mode]};`);
  const kinds = new Set<string>();
  for (const kind of FILE_KIND_ORDER) {
    const style = FILE_KINDS[kind];
    if (kinds.has(style.varName)) continue;
    kinds.add(style.varName);
    const color =
      style.slot !== null
        ? categorical[style.slot]![mode]
        : style.varName === "--viz-kind-folded"
          ? neutral.folded[mode]
          : neutral.foldedAlt[mode];
    lines.push(`${style.varName}: ${color};`);
  }
  lines.push(`--viz-directory: ${neutral.directory[mode]};`);
  lines.push(`--viz-hatch: ${neutral.hatch[mode]};`);
  lines.push(`--viz-track: ${mode === "dark" ? "rgb(214 226 240 / 0.06)" : "rgb(16 24 34 / 0.06)"};`);
  lines.push(`--viz-grid: ${mode === "dark" ? "rgb(214 226 240 / 0.06)" : "rgb(16 24 34 / 0.07)"};`);
  return lines.join("\n  ");
}

/** CSS for both themes; injected once at startup so charts reference roles, not hex. */
export function datavizCss(): string {
  return `:root, [data-theme="dark"] {\n  ${block("dark")}\n}\n[data-theme="light"] {\n  ${block("light")}\n}\n`;
}

export function installDatavizCss(doc: Document = document): void {
  if (doc.getElementById("sentinel-dataviz")) return;
  const style = doc.createElement("style");
  style.id = "sentinel-dataviz";
  style.textContent = datavizCss();
  doc.head.appendChild(style);
}
