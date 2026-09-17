import { MapPin, ShieldCheck } from "@phosphor-icons/react";
import { useState } from "react";

import type { HomeLocation } from "../../bindings/HomeLocation";
import { Button } from "../../components/Button";
import { Skeleton } from "../../components/Skeleton";
import { ErrorState } from "../../components/States";
import { describeError } from "../../lib/errors";
import { useGeoStatus, useNetwork } from "../../stores/network";
import { useSettings } from "../../stores/settings";
import { toast } from "../../stores/toasts";
import { GeoDbNotice } from "../network/GeoDbNotice";
import { useNetworkView } from "../network/viewState";
import { SettingRow, SettingsSection } from "./SettingsView";

export function validateHomeInput(label: string, lat: string, lon: string): string | null {
  const latN = Number(lat);
  const lonN = Number(lon);
  if (!label.trim()) return "Enter a name for this location.";
  if (lat.trim() === "" || !Number.isFinite(latN) || latN < -90 || latN > 90) return "Latitude must be a number between -90 and 90.";
  if (lon.trim() === "" || !Number.isFinite(lonN) || lonN < -180 || lonN > 180) return "Longitude must be a number between -180 and 180.";
  return null;
}

const inputClass =
  "num selectable h-7 rounded-[5px] border border-line bg-sunken px-2 text-[13px] text-fg placeholder:text-fg-subtle focus:border-line-strong focus:outline-none";

function HomeForm({ home }: { home: HomeLocation }) {
  const setHome = useNetwork((s) => s.setHome);
  const [label, setLabel] = useState(home.source === "userSetting" ? home.label : "");
  const [lat, setLat] = useState(home.source === "userSetting" ? String(home.lat) : "");
  const [lon, setLon] = useState(home.source === "userSetting" ? String(home.lon) : "");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const save = async (input: { label: string; lat: number; lon: number } | null) => {
    setSaving(true);
    setError(null);
    try {
      await setHome(input);
      toast({ kind: "success", title: input ? "Home location saved" : "Home location follows the time zone again" });
    } catch (e) {
      setError(describeError(e).detail);
    } finally {
      setSaving(false);
    }
  };

  return (
    <form
      className="flex flex-col gap-2"
      onSubmit={(e) => {
        e.preventDefault();
        const problem = validateHomeInput(label, lat, lon);
        if (problem) {
          setError(problem);
          return;
        }
        void save({ label: label.trim(), lat: Number(lat), lon: Number(lon) });
      }}
    >
      <div className="grid grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)_minmax(0,1fr)] gap-2">
        <label className="flex flex-col gap-1 text-[12px] text-fg-muted">
          Name
          <input className={`${inputClass} font-sans`} value={label} onChange={(e) => setLabel(e.target.value)} placeholder="Lisbon" />
        </label>
        <label className="flex flex-col gap-1 text-[12px] text-fg-muted">
          Latitude
          <input className={inputClass} inputMode="decimal" value={lat} onChange={(e) => setLat(e.target.value)} placeholder="38.72" />
        </label>
        <label className="flex flex-col gap-1 text-[12px] text-fg-muted">
          Longitude
          <input className={inputClass} inputMode="decimal" value={lon} onChange={(e) => setLon(e.target.value)} placeholder="-9.14" />
        </label>
      </div>
      {error && <p className="text-[12px] text-danger">{error}</p>}
      <div className="flex gap-2">
        <Button type="submit" size="sm" variant="primary" disabled={saving}>
          Save location
        </Button>
        {home.source === "userSetting" && (
          <Button size="sm" variant="ghost" disabled={saving} onClick={() => void save(null)}>
            Use time zone estimate
          </Button>
        )}
      </div>
    </form>
  );
}

export function NetworkSettings() {
  useGeoStatus();
  const home = useNetwork((s) => s.home);
  const homeError = useNetwork((s) => s.homeError);
  const loadHome = useNetwork((s) => s.loadHome);
  const geo = useNetwork((s) => s.geo);
  const geoError = useNetwork((s) => s.geoError);
  const loadGeo = useNetwork((s) => s.loadGeo);

  return (
    <SettingsSection title="Network map" description="Where the world map starts arcs from, and the local database that places remote addresses.">
      <div className="flex flex-col gap-3">
        <div className="flex flex-col gap-0.5">
          <span className="text-fg">Home location</span>
          <span className="text-[12px] text-fg-muted">
            Estimated from the system time zone unless you set it. It never leaves this computer.
          </span>
        </div>
        {home ? (
          <>
            <span className="flex items-center gap-2 text-[12px] text-fg-muted">
              <MapPin size={13} />
              <span className="text-fg">{home.label}</span>
              <span className="num">
                {home.lat.toFixed(2)}, {home.lon.toFixed(2)}
              </span>
              <span>{home.source === "timeZone" ? "from time zone" : "set by you"}</span>
            </span>
            <HomeForm key={`${home.source}-${home.lat}-${home.lon}`} home={home} />
          </>
        ) : homeError ? (
          <ErrorState error={homeError} subject="Home location" compact onRetry={() => void loadHome()} />
        ) : (
          <Skeleton className="h-3 w-60" />
        )}
      </div>

      <div className="flex flex-col gap-3 border-t border-line pt-5">
        <div className="flex flex-col gap-0.5">
          <span className="text-fg">IP location database</span>
          <span className="text-[12px] text-fg-muted">Used only for the map. Lookups happen on this computer.</span>
        </div>
        {geo?.state === "ready" && (
          <div className="flex flex-col gap-1 text-[12px]">
            <span className="flex items-center gap-1.5 text-signal">
              <ShieldCheck size={13} weight="fill" /> Installed{geo.buildDate ? `, built ${geo.buildDate}` : ""}
            </span>
            <span className="text-fg-subtle">{geo.attribution}</span>
          </div>
        )}
        {geo && geo.state !== "ready" && <GeoDbNotice status={geo} compact />}
        {!geo && geoError !== null && <ErrorState error={geoError} subject="The location database" compact onRetry={() => void loadGeo()} />}
        {!geo && geoError === null && <Skeleton className="h-3 w-48" />}
      </div>

      <SettingRow
        label="Firewall rules"
        detail="Addresses and ports you blocked from Sentinel. Your other firewall rules are not shown or changed."
        control={
          <Button
            size="sm"
            onClick={() => {
              useNetworkView.getState().setTab("firewall");
              useSettings.getState().setView("network");
            }}
          >
            Open firewall rules
          </Button>
        }
      />
    </SettingsSection>
  );
}
