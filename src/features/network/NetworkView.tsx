import { MapPin, X } from "@phosphor-icons/react";
import { useMemo } from "react";

import { SkeletonRows } from "../../components/Skeleton";
import { SearchField, Select } from "../../components/Field";
import { Segmented } from "../../components/Segmented";
import { ErrorState, StatePanel } from "../../components/States";
import { describeError } from "../../lib/errors";
import { formatCount, pluralize } from "../../lib/format";
import { HeaderToolbar } from "../../shell/HeaderSlot";
import { useGeoStatus, useNetwork, useNetworkFeed } from "../../stores/network";
import { ConnectionsTable, trafficNote } from "./ConnectionsTable";
import { FirewallRules } from "./FirewallRules";
import { GeoDbNotice } from "./GeoDbNotice";
import { geoKey } from "./geo";
import { isListening, socketMatcher, unmappedCounts, type ProtocolFilter, type StateFilter } from "./grouping";
import { ListeningTable } from "./ListeningTable";
import { ThroughputStrip } from "./ThroughputStrip";
import { useNetworkView, type NetworkTab } from "./viewState";
import { WorldMap } from "./WorldMap";

function NetworkToolbar() {
  const sockets = useNetwork((s) => s.sockets);
  const v = useNetworkView();
  const processes = useMemo(() => {
    const byPid = new Map<number, string>();
    for (const s of sockets) if (s.pid !== null) byPid.set(s.pid, s.processName ?? `PID ${s.pid}`);
    return [...byPid.entries()].sort((a, b) => a[1].localeCompare(b[1]));
  }, [sockets]);

  return (
    <div className="flex min-w-0 flex-1 items-center gap-2">
      <Segmented<NetworkTab>
        label="Network view"
        value={v.tab}
        onChange={v.setTab}
        options={[
          { value: "connections", label: "Connections" },
          { value: "listening", label: "Listening" },
          { value: "firewall", label: "Firewall rules" },
        ]}
      />
      {v.tab !== "firewall" && (
        <>
          <SearchField value={v.query} onChange={v.setQuery} placeholder="Search hosts, addresses, ports" label="Search connections" focusShortcut className="w-60" />
          <Select label="Protocol" value={v.protocol} onChange={(e) => v.setProtocol(e.target.value as ProtocolFilter)}>
            <option value="all">TCP and UDP</option>
            <option value="tcp">TCP</option>
            <option value="udp">UDP</option>
          </Select>
          {v.tab === "connections" && (
            <Select label="State" value={v.state} onChange={(e) => v.setState(e.target.value as StateFilter)}>
              <option value="all">Any state</option>
              <option value="established">Established</option>
              <option value="opening">Opening</option>
              <option value="closing">Closing</option>
            </Select>
          )}
          <Select label="Process" value={v.pid ?? ""} onChange={(e) => v.setPid(e.target.value ? Number(e.target.value) : null)} className="max-w-44">
            <option value="">All processes</option>
            {processes.map(([pid, name]) => (
              <option key={pid} value={pid}>
                {name} ({pid})
              </option>
            ))}
          </Select>
        </>
      )}
    </div>
  );
}

function MapPanel({ connections }: { connections: ReturnType<typeof useNetwork.getState>["sockets"] }) {
  const geo = useNetwork((s) => s.geo);
  const geoError = useNetwork((s) => s.geoError);
  const home = useNetwork((s) => s.home);
  const homeError = useNetwork((s) => s.homeError);
  const hostnames = useNetwork((s) => s.hostnames);
  const cluster = useNetworkView((s) => s.cluster);
  const selectedSocketId = useNetworkView((s) => s.selectedSocketId);
  const selectCluster = useNetworkView((s) => s.selectCluster);
  const ready = geo?.state === "ready";
  const counts = useMemo(() => unmappedCounts(connections), [connections]);
  const mapped = connections.length - counts.local - counts.unlocated;

  return (
    <div className="relative h-full">
      <WorldMap
        sockets={ready ? connections : []}
        home={home}
        hostnames={hostnames}
        selectedClusterId={cluster?.id ?? null}
        highlightSocketId={selectedSocketId}
        onSelectCluster={selectCluster}
        dimmed={!ready}
      />

      {!ready && (
        <div className="absolute inset-y-0 left-0 flex items-center pl-8">
          <div className="shadow-float rounded-[12px] bg-panel p-5">
            {geo ? (
              <GeoDbNotice status={geo} />
            ) : geoError ? (
              <ErrorState error={geoError} subject="The location database" compact />
            ) : (
              <div className="flex w-80 flex-col gap-2">
                <div className="skeleton h-3 w-40" />
                <div className="skeleton h-2.5 w-full" />
                <div className="skeleton h-2.5 w-3/4" />
              </div>
            )}
          </div>
        </div>
      )}

      {ready && (
        <div className="pointer-events-none absolute bottom-3 left-4 flex flex-col gap-0.5 text-[11px] text-fg-muted">
          <span>
            <span className="num text-fg">{formatCount(mapped)}</span> {mapped === 1 ? "connection" : "connections"} on the map
          </span>
          {counts.local > 0 && <span>{pluralize(counts.local, "connection")} to this computer or the local network, not shown</span>}
          {counts.unlocated > 0 && <span>{pluralize(counts.unlocated, "public address", "public addresses")} without a known location</span>}
        </div>
      )}
      {ready && geo.state === "ready" && (
        <span className="pointer-events-none absolute bottom-3 right-4 text-[11px] text-fg-subtle">{geo.attribution}</span>
      )}
      <div className="absolute left-4 top-3 flex flex-col items-start gap-1.5">
        {home && (
          <span className="flex items-center gap-1.5 text-[11px] text-fg-muted" title={home.source === "timeZone" ? "Estimated from the system time zone. Change it in Settings." : "Set in Settings"}>
            <MapPin size={12} />
            {home.label}
            {home.source === "timeZone" ? ", from time zone" : ""}
          </span>
        )}
        {!home && homeError !== null && (
          <span className="text-[11px] text-fg-subtle">Home location unavailable: {describeError(homeError).detail}</span>
        )}
        {cluster && (
          <button
            type="button"
            onClick={() => selectCluster(null)}
            className="shadow-float flex items-center gap-1.5 rounded-full bg-raised py-1 pl-2.5 pr-2 text-[12px] text-fg hover:bg-panel"
          >
            Showing {cluster.label}
            <X size={11} className="text-fg-muted" />
          </button>
        )}
      </div>
    </div>
  );
}

export function NetworkView() {
  useNetworkFeed();
  useGeoStatus();
  const sockets = useNetwork((s) => s.sockets);
  const status = useNetwork((s) => s.status);
  const error = useNetwork((s) => s.error);
  const load = useNetwork((s) => s.load);
  const trafficSource = useNetwork((s) => s.trafficSource);
  const hostnames = useNetwork((s) => s.hostnames);
  const tab = useNetworkView((s) => s.tab);
  const query = useNetworkView((s) => s.query);
  const protocol = useNetworkView((s) => s.protocol);
  const stateFilter = useNetworkView((s) => s.state);
  const pid = useNetworkView((s) => s.pid);
  const cluster = useNetworkView((s) => s.cluster);

  const { connections, listening } = useMemo(() => {
    const match = socketMatcher({ query, protocol, state: tab === "listening" ? "all" : stateFilter, pid }, (ip) => hostnames.get(ip) ?? null);
    const connections = [];
    const listening = [];
    for (const s of sockets) {
      if (!match(s)) continue;
      if (isListening(s)) listening.push(s);
      else connections.push(s);
    }
    return { connections, listening };
  }, [sockets, query, protocol, stateFilter, pid, hostnames, tab]);

  const tableSockets = useMemo(() => {
    if (!cluster) return connections;
    const keys = new Set(cluster.keys);
    return connections.filter((s) => {
      const k = geoKey(s);
      return k !== null && s.remoteScope === "public" && keys.has(k);
    });
  }, [connections, cluster]);

  const note = trafficNote(trafficSource);
  const loading = sockets.length === 0 && (status === "idle" || status === "loading");
  const failed = sockets.length === 0 && status === "error";

  return (
    <div className="flex h-full flex-col">
      <HeaderToolbar>
        <NetworkToolbar />
      </HeaderToolbar>

      {tab === "connections" && (
        <>
          <div className="h-[44%] min-h-[280px] shrink-0 border-b border-line">
            <MapPanel connections={connections} />
          </div>
          <div className="shrink-0 border-b border-line px-5 py-3">
            <ThroughputStrip />
          </div>
          <div className="min-h-0 flex-1">
            {loading && <SkeletonRows rows={10} columns={[3, 2.5, 1, 0.8, 1.2, 1.6, 2]} />}
            {failed && (
              <StatePanel>
                <ErrorState error={error} subject="Network connections" onRetry={() => void load()} />
              </StatePanel>
            )}
            {!loading && !failed && (
              <ConnectionsTable
                sockets={tableSockets}
                trafficSource={trafficSource}
                hostnames={hostnames}
                forceExpand={cluster !== null || query.trim() !== "" || pid !== null}
              />
            )}
          </div>
          {note && !failed && <p className="shrink-0 border-t border-line px-5 py-2 text-[11px] text-fg-subtle">{note}</p>}
        </>
      )}

      {tab === "listening" && (
        <div className="min-h-0 flex-1">
          {loading && <SkeletonRows rows={12} columns={[1, 0.8, 3, 3, 2]} rowHeight={32} />}
          {failed && (
            <StatePanel>
              <ErrorState error={error} subject="Listening ports" onRetry={() => void load()} />
            </StatePanel>
          )}
          {!loading && !failed && <ListeningTable sockets={listening} />}
        </div>
      )}

      {tab === "firewall" && (
        <div className="min-h-0 flex-1">
          <FirewallRules />
        </div>
      )}
    </div>
  );
}
