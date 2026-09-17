// Searchable list of every term Sentinel explains, generated from the same glossary module the
// `?`/`i` tooltips read from, so the two can never drift apart.
import { useMemo, useState } from "react";

import { SearchField } from "../../components/Field";
import { EmptyState, StatePanel } from "../../components/States";
import { GLOSSARY, GLOSSARY_IDS, resolveDefinition } from "../../lib/glossary";
import { useResources } from "../../stores/resources";

export function Glossary() {
  const [query, setQuery] = useState("");
  const cores = useResources((s) => s.systemInfo?.logicalCores ?? null);

  const rows = useMemo(() => {
    const terms = GLOSSARY_IDS.map((id) => ({ id, entry: GLOSSARY[id] }));
    const q = query.trim().toLowerCase();
    if (!q) return terms;
    return terms.filter(({ entry }) => {
      const definition = resolveDefinition(entry, { cores });
      return `${entry.term}\n${definition}\n${entry.unit ?? ""}`.toLowerCase().includes(q);
    });
  }, [query, cores]);

  return (
    <div className="flex flex-col gap-4">
      <SearchField value={query} onChange={setQuery} placeholder="Search terms" label="Search glossary" className="w-full max-w-72" />
      {rows.length === 0 ? (
        <StatePanel className="h-32">
          <EmptyState title={`No terms match "${query}"`} />
        </StatePanel>
      ) : (
        <dl className="flex flex-col divide-y divide-line">
          {rows.map(({ id, entry }) => (
            <div key={id} className="grid grid-cols-[160px_minmax(0,1fr)] gap-4 py-3">
              <dt className="font-medium text-fg">{entry.term}</dt>
              <dd className="flex flex-col gap-1 text-fg-muted">
                <span>{resolveDefinition(entry, { cores })}</span>
                {entry.unit && <span className="text-[12px] text-fg-subtle">Unit: {entry.unit}</span>}
                {entry.source && <span className="text-[12px] text-fg-subtle">{entry.source}</span>}
                {entry.caveat && <span className="text-[12px] text-warn">{entry.caveat}</span>}
              </dd>
            </div>
          ))}
        </dl>
      )}
    </div>
  );
}
