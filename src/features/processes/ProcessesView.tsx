import { ListBullets, TreeStructure } from "@phosphor-icons/react";
import { useMemo } from "react";

import { SearchField, Select } from "../../components/Field";
import { Segmented } from "../../components/Segmented";
import { formatCount } from "../../lib/format";
import { useProcesses, useProcessFeed } from "../../stores/processes";
import { HeaderToolbar } from "../../shell/HeaderSlot";
import type { StatusFilter } from "./columns";
import { ProcessDrawer } from "./ProcessDrawer";
import { ProcessTable } from "./ProcessTable";
import { useProcessView, type ProcessMode } from "./viewState";

function ProcessToolbar() {
  const processes = useProcesses((s) => s.processes);
  const mode = useProcessView((s) => s.mode);
  const setMode = useProcessView((s) => s.setMode);
  const query = useProcessView((s) => s.query);
  const setQuery = useProcessView((s) => s.setQuery);
  const status = useProcessView((s) => s.status);
  const setStatus = useProcessView((s) => s.setStatus);
  const user = useProcessView((s) => s.user);
  const setUser = useProcessView((s) => s.setUser);

  const users = useMemo(
    () => [...new Set(processes.map((p) => p.user).filter((u): u is string => Boolean(u)))].sort((a, b) => a.localeCompare(b)),
    [processes],
  );

  return (
    <div className="flex min-w-0 flex-1 items-center gap-2">
      <Segmented<ProcessMode>
        label="Layout"
        value={mode}
        onChange={setMode}
        options={[
          { value: "list", label: <><ListBullets size={13} /> List</> },
          { value: "tree", label: <><TreeStructure size={13} /> Tree</> },
        ]}
      />
      <SearchField value={query} onChange={setQuery} placeholder="Search processes" label="Search processes" focusShortcut className="w-64" />
      <Select label="Status" value={status} onChange={(e) => setStatus(e.target.value as StatusFilter)}>
        <option value="all">All statuses</option>
        <option value="running">Running</option>
        <option value="sleeping">Sleeping or idle</option>
        <option value="stopped">Stopped</option>
        <option value="zombie">Zombie or exited</option>
      </Select>
      <Select label="User" value={user ?? ""} onChange={(e) => setUser(e.target.value || null)} className="max-w-40">
        <option value="">All users</option>
        {users.map((u) => (
          <option key={u} value={u}>
            {u}
          </option>
        ))}
      </Select>
      {processes.length > 0 && (
        <span className="ml-1 whitespace-nowrap text-[12px] text-fg-muted">
          <span className="num">{formatCount(processes.length)}</span> processes
        </span>
      )}
    </div>
  );
}

export function ProcessesView() {
  useProcessFeed();
  return (
    <div className="relative h-full overflow-hidden">
      <HeaderToolbar>
        <ProcessToolbar />
      </HeaderToolbar>
      <ProcessTable />
      <ProcessDrawer />
    </div>
  );
}
