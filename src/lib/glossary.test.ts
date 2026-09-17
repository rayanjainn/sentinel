// Glossary completeness: every `?`/`i` tooltip in the UI must resolve to a real entry (a typo or a
// renamed key fails this immediately), and every field docs/PLAIN_LANGUAGE.md §3 requires explained
// must actually be in the glossary. A field added to the UI without a matching InfoTip call is not
// caught here (that needs a real DOM crawl); this is the static-analysis half of that guarantee.
//
// This is the one file in `src/` that reads the source tree from Node rather than running in the
// browser, so it needs `@types/node`'s ambient declarations that the project's `tsconfig.json`
// otherwise leaves out (its `types` array is deliberately browser-only elsewhere).
/// <reference types="node" />
import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

import { GLOSSARY, GLOSSARY_IDS } from "./glossary";

function listSourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === "bindings" || entry.name.startsWith(".")) continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) out.push(...listSourceFiles(full));
    else if (/\.(ts|tsx)$/.test(entry.name) && !entry.name.endsWith(".test.ts") && !entry.name.endsWith(".test.tsx")) {
      out.push(full);
    }
  }
  return out;
}

/**
 * Every glossary id mentioned anywhere in the source as a quoted string literal — whether a
 * direct `<InfoTip id="x">` or a lookup table feeding one dynamically (`columns.ts`'s
 * `glossaryId: "x"`, a `Record<Key, GlossaryId>` map). TypeScript's own `GlossaryId` union already
 * rejects a typo or a renamed key at every one of those call sites at compile time; this check's
 * job is the thing the compiler does not do: catching an entry nothing ever renders.
 */
function referencedGlossaryIds(): Set<string> {
  const root = path.resolve(process.cwd(), "src");
  let all = "";
  for (const file of listSourceFiles(root)) {
    if (path.basename(file) === "glossary.ts") continue;
    all += fs.readFileSync(file, "utf8");
  }
  return new Set(GLOSSARY_IDS.filter((id) => all.includes(`"${id}"`)));
}

// Every field docs/PLAIN_LANGUAGE.md §3 explicitly lists as required coverage.
const MUST_COVER = [
  "cpuPercent",
  "memoryRss",
  "memoryVirtual",
  "threadCount",
  "fdCount",
  "nice",
  "pid",
  "ppid",
  "processStatus",
  "loadAverage",
  "swap",
  "compressedMemory",
  "wiredMemory",
  "cachedMemory",
  "temperature",
  "fanSpeed",
  "throughputRate",
  "connectionRate",
  "cumulativeBytes",
  "connectionState",
  "listeningPort",
  "remoteHost",
  "geoLocation",
  "allocatedSize",
  "smallFilesNode",
  "otherItemsNode",
  "duplicateGroup",
  "cleanupAppCache",
  "cleanupLogs",
  "cleanupTrash",
  "cleanupOldDownloads",
  "cleanupBuildArtifacts",
  "cleanupPackageManagerCache",
  "cleanupDeveloperCache",
  "moveToTrash",
  "firewallRuleGlossary",
] as const;

describe("glossary completeness", () => {
  it("covers every field docs/PLAIN_LANGUAGE.md §3 requires", () => {
    const missing = MUST_COVER.filter((id) => !(id in GLOSSARY));
    expect(missing).toEqual([]);
  });

  it("has no orphaned entry — every term in the module is actually rendered somewhere", () => {
    const referenced = referencedGlossaryIds();
    const orphaned = GLOSSARY_IDS.filter((id) => !referenced.has(id));
    expect(orphaned).toEqual([]);
  });

  it("never renders an empty definition", () => {
    for (const id of GLOSSARY_IDS) {
      const entry = GLOSSARY[id];
      const text = typeof entry.definition === "function" ? entry.definition({ cores: 8 }) : entry.definition;
      expect(text.length, `${id} has an empty definition`).toBeGreaterThan(0);
    }
  });
});
