import { defineCollection } from "astro:content";
import { glob } from "astro/loaders";
import { z } from "astro/zod";

export const DOC_SECTIONS = ["Get started", "Modules", "Trust and privacy"] as const;

const docs = defineCollection({
  // `modules/processes.md` gets the id `modules/processes` and the route /docs/modules/processes/.
  loader: glob({ pattern: "**/*.md", base: "./src/content/docs" }),
  schema: z.object({
    title: z.string(),
    /** Shorter label for the sidebar. Defaults to `title`. */
    navTitle: z.string().optional(),
    description: z.string(),
    section: z.enum(DOC_SECTIONS),
    order: z.number().int(),
  }),
});

export const collections = { docs };
