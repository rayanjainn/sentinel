// Shared helpers for the site figures: drawing shorthands, and the writer that
// turns one figure into (a) a static HTML fragment for a site, (b) a classic
// script that brings it to life, and (c) a standalone dev page for checking.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import * as k from "./kit/iso-kit.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
// Where a build writes: scripts and styles (FIG_OUT), the HTML fragment the
// site includes (FIG_FRAG), and standalone check pages (FIG_DEV, optional).
export const OUT = process.env.FIG_OUT || join(HERE, "out");
const FRAG = process.env.FIG_FRAG || OUT;
const DEV = process.env.FIG_DEV || (process.env.FIG_OUT ? "" : join(HERE, "dev"));

export { k };

export function tools(P) {
  const at = (point) => k.iso(point, P);
  const S = (paths, style) => k.solidSvg(paths, style);
  const L = (d, style) => k.lineSvg(d, style);
  const D = (points, style) => k.dotsSvg(points, style);
  const halo = (plan, z, h, blur) =>
    `<path class="iso-halo" d="${k.haloOf(plan, z, h, P)}"${blur ? ` style="filter:blur(${blur}px)"` : ""}/>`;
  const byDepth = (points) =>
    [...points].sort((a, b) => k.depthOf([a[0], a[1], a[2] ?? 0], P) - k.depthOf([b[0], b[1], b[2] ?? 0], P));
  const depth = (point) => k.depthOf(point, P);
  // A label plate: thin slab, inset groove, three rule lines standing for print, two screws.
  const labelPlate = (plan, z) => {
    const out = [S(k.slabOf(plan, z, 0.8, P, 3), { tone: "lo", crease: "none" })];
    out.push(L(k.planOutline(k.insetPlan(plan, 1.2), z + 0.8, P, 3), { tone: "faint" }));
    const x0 = plan.x + 3.2;
    const rows = [0.32, 0.5, 0.68];
    const lens = [0.62, 0.48, 0.3];
    rows.forEach((f, i) => {
      const y = plan.y + plan.d * f;
      out.push(L(k.lineOnTop([x0, y], [x0 + (plan.w - 6.4) * lens[i], y], z + 0.8, P), { tone: "lo", free: true }));
    });
    out.push(D([at([plan.x + plan.w - 2.4, plan.y + plan.d / 2, z + 0.8])], { size: 0.4 }));
    return out.join("");
  };
  // Base plate on turned feet, with groove, corner screws and soft halo.
  const basePlate = (plan, h, footH, opts = {}) => {
    const out = [halo(plan, -h - footH, h)];
    for (const [x, y] of byDepth(k.corners(plan, opts.footInset ?? 20))) {
      out.push(S(k.cylinder(x, y, 7, -h - footH, 1.6, P, 28), { tone: "lo" }));
      out.push(S(k.cylinder(x, y, 5.4, -h - footH + 1.6, footH - 1.6, P, 28), { tone: "mid" }));
    }
    out.push(S(k.slabOf(plan, -h, h, P, 8, 1.6), { tone: "mid" }));
    out.push(L(k.planOutline(k.insetPlan(plan, 5), 0, P), { tone: "lo" }));
    const screws = k.corners(plan, 10);
    out.push(L(screws.map(([x, y]) => k.ring(x, y, 2.2, 0, P, 16)).join(""), { tone: "lo" }));
    out.push(D(screws.map(([x, y]) => at([x, y, 0])), { size: 0.5 }));
    return out.join("");
  };
  // A ruler strip with ticks rooted on its near edge.
  const ruler = (strip, z, axis = "x") => {
    const out = [S(k.slabOf(strip, z, 0.6, P, 3), { tone: "lo", crease: "none" })];
    const t =
      axis === "x"
        ? k.topTicks(strip.x + 3, strip.x + strip.w - 3, 4, 5, strip.y + 0.8, z + 0.6, [1.8, 3.4], P)
        : k.topTicks(strip.y + 3, strip.y + strip.d - 3, 4, 5, strip.x + 0.8, z + 0.6, [1.8, 3.4], P, "y");
    out.push(L(t.minor, { tone: "lo" }), L(t.major, { tone: "mid" }));
    return out.join("");
  };
  return { at, S, L, D, halo, byDepth, depth, labelPlate, basePlate, ruler };
}

function kitClassic() {
  const src = readFileSync(join(HERE, "kit", "iso-kit.mjs"), "utf8");
  const names = [...src.matchAll(/^export (?:function|const) ([A-Za-z_$][\w$]*)/gm)].map((m) => m[1]);
  const body = src.replace(/^export /gm, "");
  return `/* iso-kit (Anatomy, MIT, see figures/kit/LICENSE) */\n(function(){\n${body}\nwindow.isoKit={${names.join(",")}};\n})();\n`;
}

/**
 * Writes one figure.
 * live: source of a function body run once per figure root, with `k` (the kit),
 * `D` (the JSON data) and `root` (the <figure> wrapper) in scope.
 */
export function emit({ name, title, svg, plate, live, data, css = "" }) {
  mkdirSync(OUT, { recursive: true });
  mkdirSync(FRAG, { recursive: true });
  const fragment = `<div class="iso iso-mount" data-figure="${name}">${k.plateHtml({ ...plate, body: `<div class="iso-stage" tabindex="0" ${plate.aria}>${svg}</div>` })}</div>`;
  const script = `/* ${title}: figure script, built by figures/${name}.mjs */\n(function(){\n"use strict";\nconst k = window.isoKit;\nconst D = ${JSON.stringify(data)};\nfunction mount(root){\n${live}\n}\nfunction start(){document.querySelectorAll('[data-figure="${name}"]').forEach(function(root){if(!root.dataset.live){root.dataset.live="1";mount(root);}});}\nif(document.readyState==="loading")document.addEventListener("DOMContentLoaded",start);else start();\n})();\n`;
  writeFileSync(join(FRAG, `${name}.html`), fragment + "\n");
  writeFileSync(join(OUT, `${name}.js`), script);
  writeFileSync(join(OUT, "iso-kit.js"), kitClassic());
  writeFileSync(join(OUT, "iso.css"), k.ISO_CSS.trim() + "\n");
  writeFileSync(join(OUT, `${name}.css`), css.trim() + "\n");
  if (!DEV) {
    console.log("wrote", name);
    return;
  }
  mkdirSync(DEV, { recursive: true });
  for (const theme of ["dark", "light"]) {
    const page = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>${title}</title><style>${k.ISO_CSS}*{box-sizing:border-box}body{margin:0;background:var(--anatomy-page);color:var(--anatomy-text);font:14px/1.6 system-ui,sans-serif}.iso-page{width:min(720px,100% - 32px);margin:64px auto 96px}${css}</style></head><body class="iso" data-theme="${theme}"><main class="iso-page">${fragment.replace('class="iso iso-mount"', `class="iso iso-mount" data-theme="${theme}"`)}</main><script>${kitClassic().replace(/<\/(script)/gi, "<\\/$1")}</script><script>${script.replace(/<\/(script)/gi, "<\\/$1")}</script></body></html>`;
    writeFileSync(join(DEV, `${name}${theme === "light" ? "-light" : ""}.html`), page);
  }
  console.log("wrote", name, `(${(svg.length / 1024).toFixed(0)} KB svg)`);
}
