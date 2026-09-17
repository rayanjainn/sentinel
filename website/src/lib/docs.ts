import { getCollection, type CollectionEntry } from "astro:content";
import { DOC_SECTIONS } from "../content.config";
import { href } from "./site";

export type DocEntry = CollectionEntry<"docs">;

export interface DocSection {
  title: (typeof DOC_SECTIONS)[number];
  entries: DocEntry[];
}

/** Docs in reading order: by section, then by `order` within it. */
export async function getSortedDocs(): Promise<DocEntry[]> {
  const docs = await getCollection("docs");
  return docs.sort(
    (a, b) =>
      DOC_SECTIONS.indexOf(a.data.section) - DOC_SECTIONS.indexOf(b.data.section) ||
      a.data.order - b.data.order,
  );
}

export async function getDocSections(): Promise<DocSection[]> {
  const docs = await getSortedDocs();
  return DOC_SECTIONS.map((title) => ({
    title,
    entries: docs.filter((entry) => entry.data.section === title),
  })).filter((section) => section.entries.length > 0);
}

export function docHref(id: string): string {
  return href(`docs/${id}/`);
}
