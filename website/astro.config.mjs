// @ts-check
import { fileURLToPath } from "node:url";
import sitemap from "@astrojs/sitemap";
import { defineConfig } from "astro/config";

// The site shares design tokens with the desktop app (../src/styles/tokens.css).
const repoRoot = fileURLToPath(new URL("..", import.meta.url));
const appStyles = fileURLToPath(new URL("../src/styles", import.meta.url));

export default defineConfig({
  // Project Pages site: https://rayanjainn.github.io/sentinel/
  site: "https://rayanjainn.github.io",
  base: "/sentinel",
  trailingSlash: "always",
  output: "static",
  // Astro 7 defaults to "jsx" whitespace handling, which drops spaces between inline elements.
  compressHTML: true,
  integrations: [sitemap()],
  vite: {
    resolve: {
      alias: { "@app-styles": appStyles },
    },
    server: {
      // Allow the dev server to read the shared tokens outside website/.
      fs: { allow: [repoRoot] },
    },
  },
});
