// Rebuilds the site's isometric figure: `node website/figures/build.mjs`.
// Writes the script and styles to website/public/figures/ and the HTML
// fragment to website/src/figures/, which Sampler.astro inlines. The output is
// committed, so `pnpm build` needs nothing from here.
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
process.env.FIG_OUT = join(HERE, "..", "public", "figures");
process.env.FIG_FRAG = join(HERE, "..", "src", "figures");
await import("./sentinel-recorder.mjs");
