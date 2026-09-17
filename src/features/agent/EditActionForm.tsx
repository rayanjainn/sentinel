import { FolderOpen } from "@phosphor-icons/react";
import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";

import type { Action } from "../../bindings/Action";
import type { ErrorPayload } from "../../bindings/ErrorPayload";
import type { PlanAction } from "../../bindings/PlanAction";
import type { TrafficDirection } from "../../bindings/TrafficDirection";
import { Button } from "../../components/Button";
import { Checkbox } from "../../components/Field";
import { Segmented } from "../../components/Segmented";
import { ErrorState } from "../../components/States";
import { formatNice } from "../../lib/format";
import { useAgent } from "./store";

/** Actions whose parameters a person can narrow or adjust before approving. */
export function isEditable(action: Action): boolean {
  return (
    action.type === "trashPaths" ||
    action.type === "movePaths" ||
    action.type === "setProcessPriority" ||
    action.type === "addFirewallRule"
  );
}

function PathChecklist({
  paths,
  kept,
  onToggle,
}: {
  paths: string[];
  kept: Set<string>;
  onToggle: (path: string, keep: boolean) => void;
}) {
  return (
    <ul className="max-h-48 divide-y divide-line overflow-y-auto rounded-[5px] border border-line bg-sunken">
      {paths.map((path) => (
        <li key={path} className="flex items-center gap-2 px-2.5 py-1.5">
          <Checkbox checked={kept.has(path)} onChange={(keep) => onToggle(path, keep)} label={`Keep ${path}`} />
          <span className="num selectable min-w-0 break-all text-[12px] text-fg">{path}</span>
        </li>
      ))}
    </ul>
  );
}

export function EditActionForm({ planId, item, onDone }: { planId: string; item: PlanAction; onDone: () => void }) {
  const revise = useAgent((s) => s.revise);
  const original = item.preview.action;
  const [draft, setDraft] = useState<Action>(original);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<ErrorPayload | null>(null);

  const save = async () => {
    setSaving(true);
    setError(null);
    const failure = await revise(planId, item.id, draft);
    setSaving(false);
    if (failure) setError(failure);
    else onDone();
  };

  let fields = null;
  let valid = true;
  if ((draft.type === "trashPaths" || draft.type === "movePaths") && (original.type === "trashPaths" || original.type === "movePaths")) {
    const kept = new Set(draft.paths);
    valid = draft.paths.length > 0;
    const toggle = (path: string, keep: boolean) => {
      const paths = original.paths.filter((p) => (p === path ? keep : kept.has(p)));
      setDraft({ ...draft, paths });
    };
    fields = (
      <div className="flex flex-col gap-2">
        <span className="text-[12px] text-fg-muted">
          Uncheck anything that should stay where it is. {draft.paths.length} of {original.paths.length} kept.
        </span>
        <PathChecklist paths={original.paths} kept={kept} onToggle={toggle} />
        {draft.type === "movePaths" && (
          <div className="flex flex-col gap-1.5">
            <span className="text-[12px] text-fg-muted">Destination folder</span>
            <div className="flex items-center gap-2">
              <span className="num selectable min-w-0 flex-1 truncate rounded-[5px] border border-line bg-sunken px-2.5 py-1 text-[12px] text-fg" title={draft.destinationDir}>
                {draft.destinationDir}
              </span>
              <Button
                size="sm"
                icon={<FolderOpen size={13} />}
                onClick={async () => {
                  const chosen = await open({ directory: true, multiple: false, defaultPath: draft.destinationDir });
                  if (typeof chosen === "string") setDraft({ ...draft, destinationDir: chosen });
                }}
              >
                Choose folder
              </Button>
            </div>
          </div>
        )}
      </div>
    );
  } else if (draft.type === "setProcessPriority") {
    fields = (
      <label className="flex flex-col gap-2">
        <span className="flex items-baseline justify-between text-[12px] text-fg-muted">
          Priority
          <span className="num text-fg">{formatNice(draft.nice)}</span>
        </span>
        <input
          type="range"
          min={-20}
          max={19}
          step={1}
          value={draft.nice}
          onChange={(e) => setDraft({ ...draft, nice: Number(e.target.value) })}
          className="accent-[var(--signal)]"
        />
        <span className="flex justify-between text-[11px] text-fg-subtle">
          <span>Highest (needs administrator)</span>
          <span>Lowest</span>
        </span>
      </label>
    );
  } else if (draft.type === "addFirewallRule") {
    fields = (
      <div className="flex items-center justify-between gap-3">
        <span className="text-[12px] text-fg-muted">Block traffic</span>
        <Segmented<TrafficDirection>
          label="Traffic direction"
          size="sm"
          value={draft.direction}
          onChange={(direction) => setDraft({ ...draft, direction })}
          options={[
            { value: "inbound", label: "Inbound" },
            { value: "outbound", label: "Outbound" },
            { value: "both", label: "Both" },
          ]}
        />
      </div>
    );
  }

  if (!fields) return null;
  return (
    <div className="flex flex-col gap-3 rounded-[5px] border border-line-strong bg-panel p-3">
      {fields}
      {error && <ErrorState error={error} compact />}
      <div className="flex justify-end gap-2">
        <Button size="sm" variant="ghost" onClick={onDone} disabled={saving}>
          Cancel
        </Button>
        <Button size="sm" variant="primary" onClick={() => void save()} disabled={!valid || saving}>
          {saving ? "Updating preview" : "Update preview"}
        </Button>
      </div>
    </div>
  );
}
