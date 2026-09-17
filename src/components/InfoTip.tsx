// A small "i" affordance that explains one field, reading from the glossary — the single source
// of truth also used by the searchable list in Settings. See docs/PLAIN_LANGUAGE.md §3.
import { Info } from "@phosphor-icons/react";

import { GLOSSARY, resolveDefinition, type GlossaryId } from "../lib/glossary";
import { useResources } from "../stores/resources";
import { Tip } from "./Tip";

export function InfoTip({ id, className }: { id: GlossaryId; className?: string }) {
  const entry = GLOSSARY[id];
  const cores = useResources((s) => s.systemInfo?.logicalCores ?? null);
  if (!entry) return null;
  const definition = resolveDefinition(entry, { cores });

  return (
    <Tip
      label={`What is ${entry.term}?`}
      icon={<Info size={13} weight="bold" />}
      className={className}
      content={
        <>
          <span className="font-medium text-fg">{entry.term}</span>
          <span className="text-fg-muted">{definition}</span>
          {entry.unit && <span className="text-fg-subtle">Unit: {entry.unit}</span>}
          {entry.source && <span className="text-fg-subtle">{entry.source}</span>}
          {entry.caveat && <span className="text-warn">{entry.caveat}</span>}
        </>
      }
    />
  );
}
