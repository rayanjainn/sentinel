// The world view: live remote endpoints on an Equal Earth map, with great-circle arcs from this
// machine's approximate home location.
import { geoEqualEarth, geoGraticule10, geoPath, type GeoProjection } from "d3-geo";
import type { Feature, FeatureCollection, MultiLineString } from "geojson";
import { memo, useMemo, useRef, useState } from "react";
import { feature, mesh } from "topojson-client";
import countriesTopology from "world-atlas/countries-110m.json";

import type { HomeLocation } from "../../bindings/HomeLocation";
import type { SocketEntry } from "../../bindings/SocketEntry";
import { cx } from "../../lib/cx";
import { formatCount, formatRate, pluralize } from "../../lib/format";
import { useElementSize, useOnScreen, usePrefersReducedMotion } from "../../lib/motion";
import { formatEndpoint, TCP_STATE_LABEL } from "../../lib/net";
import { hostFor } from "../../stores/network";
import { clusterEndpoints, markerRadius, type EndpointCluster } from "./geo";
import type { ClusterSelection } from "./viewState";

const selectionOf = (c: EndpointCluster): ClusterSelection => ({ id: c.id, label: c.label, keys: c.keys });

type AtlasTopology = Parameters<typeof mesh>[0];
type AtlasObject = NonNullable<Parameters<typeof mesh>[1]>;
const topology = countriesTopology as unknown as AtlasTopology;
const objects = topology.objects as Record<"countries" | "land", AtlasObject>;
const land = feature(topology, objects.land) as unknown as FeatureCollection;
const borders = mesh(topology, objects.countries, (a, b) => a !== b) as MultiLineString;
const graticule = geoGraticule10();

// Extent used to fit the projection: the inhabited latitudes, so Antarctica does not waste height.
const FIT_EXTENT: Feature = {
  type: "Feature",
  properties: {},
  geometry: {
    type: "MultiPoint",
    coordinates: [
      [-180, 0],
      [180, 0],
      [0, 83],
      [0, -56],
      [-150, 70],
      [150, 70],
      [-150, -50],
      [150, -50],
    ],
  },
};

const MAX_PACKETS = 18;
const MAX_TOOLTIP_ROWS = 5;

function hash(text: string): number {
  let h = 0;
  for (let i = 0; i < text.length; i += 1) h = (h * 31 + text.charCodeAt(i)) | 0;
  return Math.abs(h);
}

function makeProjection(width: number, height: number): GeoProjection {
  return geoEqualEarth().fitExtent(
    [
      [12, 12],
      [width - 12, height - 12],
    ],
    FIT_EXTENT,
  );
}

const BaseMap = memo(function BaseMap({ width, height }: { width: number; height: number }) {
  const paths = useMemo(() => {
    const path = geoPath(makeProjection(width, height));
    return {
      sphere: path({ type: "Sphere" }) ?? "",
      graticule: path(graticule) ?? "",
      land: path(land) ?? "",
      borders: path(borders) ?? "",
    };
  }, [width, height]);
  return (
    <g aria-hidden>
      <path d={paths.sphere} fill="var(--sunken)" stroke="var(--line)" strokeWidth={1} />
      <path d={paths.graticule} fill="none" stroke="var(--line)" strokeWidth={0.6} />
      <path d={paths.land} fill="color-mix(in srgb, var(--fg) 9%, var(--sunken))" />
      <path d={paths.borders} fill="none" stroke="var(--line-strong)" strokeWidth={0.5} />
    </g>
  );
});

interface Tooltip {
  cluster: EndpointCluster;
}

function ClusterTooltip({
  cluster,
  width,
  hostnames,
}: {
  cluster: EndpointCluster;
  width: number;
  hostnames: Map<string, string | null>;
}) {
  const rows = cluster.sockets.slice(0, MAX_TOOLTIP_ROWS);
  const left = cluster.x > width - 300;
  return (
    <div
      role="tooltip"
      className="shadow-float pointer-events-none absolute z-20 flex w-[280px] flex-col gap-2 rounded-[8px] bg-raised px-3 py-2.5 text-[12px]"
      style={{
        left: left ? undefined : cluster.x + 16,
        right: left ? width - cluster.x + 16 : undefined,
        top: Math.max(8, cluster.y - 24),
      }}
    >
      <div className="flex flex-col gap-0.5">
        <span className="font-medium text-fg">{cluster.label}</span>
        <span className="text-fg-muted">
          {pluralize(cluster.sockets.length, "connection")} to {pluralize(cluster.endpoints, "address", "addresses")}
          {cluster.bps > 0 ? `, ${formatRate(cluster.bps)}` : ""}
        </span>
      </div>
      <ul className="flex flex-col gap-1.5 border-t border-line pt-2">
        {rows.map((s) => {
          const host = hostFor(s, hostnames);
          return (
            <li key={s.id} className="flex flex-col">
              <div className="flex items-baseline justify-between gap-2">
                <span className="truncate text-fg">{host ?? s.remoteAddr}</span>
                <span className="shrink-0 text-fg-muted">{s.state ? TCP_STATE_LABEL[s.state] : s.protocol.toUpperCase()}</span>
              </div>
              <div className="flex items-baseline justify-between gap-2 text-fg-subtle">
                <span className="num truncate">{formatEndpoint(s.remoteAddr, s.remotePort, s.family)}</span>
                <span className="shrink-0 truncate">{s.processName ?? (s.pid !== null ? `PID ${s.pid}` : "Unknown process")}</span>
              </div>
            </li>
          );
        })}
      </ul>
      {cluster.sockets.length > rows.length && (
        <span className="text-fg-subtle">{formatCount(cluster.sockets.length - rows.length)} more in the table below</span>
      )}
    </div>
  );
}

export function WorldMap({
  sockets,
  home,
  hostnames,
  selectedClusterId,
  highlightSocketId,
  onSelectCluster,
  dimmed = false,
  className,
}: {
  sockets: SocketEntry[];
  home: HomeLocation | null;
  hostnames: Map<string, string | null>;
  selectedClusterId: string | null;
  highlightSocketId: string | null;
  onSelectCluster: (cluster: ClusterSelection | null) => void;
  dimmed?: boolean;
  className?: string;
}) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const { width, height } = useElementSize(wrapRef);
  const onScreen = useOnScreen(wrapRef);
  const reduced = usePrefersReducedMotion();
  const [hovered, setHovered] = useState<Tooltip | null>(null);

  const projection = useMemo(() => (width > 0 && height > 0 ? makeProjection(width, height) : null), [width, height]);

  const clusters = useMemo(() => {
    if (!projection) return [];
    return clusterEndpoints(sockets, (p) => projection(p) ?? null, 10);
  }, [sockets, projection]);

  const homePoint = useMemo(() => (projection && home ? projection([home.lon, home.lat]) : null), [projection, home]);

  const arcs = useMemo(() => {
    if (!projection || !home) return [];
    const path = geoPath(projection);
    return clusters.map((c) => ({
      id: c.id,
      d: path({ type: "LineString", coordinates: [[home.lon, home.lat], [c.lon, c.lat]] }) ?? "",
      cluster: c,
    }));
  }, [clusters, projection, home]);

  const packets = useMemo(
    () =>
      [...arcs]
        .filter((a) => a.d && Math.hypot(a.cluster.x - (homePoint?.[0] ?? 0), a.cluster.y - (homePoint?.[1] ?? 0)) > 18)
        .sort((a, b) => b.cluster.bps - a.cluster.bps || b.cluster.sockets.length - a.cluster.sockets.length)
        .slice(0, MAX_PACKETS),
    [arcs, homePoint],
  );

  const highlightClusterId = useMemo(() => {
    if (!highlightSocketId) return null;
    return clusters.find((c) => c.sockets.some((s) => s.id === highlightSocketId))?.id ?? null;
  }, [clusters, highlightSocketId]);

  const maxCount = clusters.reduce((m, c) => Math.max(m, c.sockets.length), 1);
  const activeId = selectedClusterId ?? highlightClusterId;

  return (
    <div
      ref={wrapRef}
      className={cx("relative h-full w-full overflow-hidden", className)}
      data-paused={!onScreen}
      onPointerLeave={() => setHovered(null)}
    >
      {width > 0 && height > 0 && (
        <>
          <svg
            width={width}
            height={height}
            role="img"
            aria-label={`World map of ${clusters.length} remote locations`}
            className={cx("block transition-opacity duration-300", dimmed && "opacity-35")}
            onClick={() => onSelectCluster(null)}
          >
            <BaseMap width={width} height={height} />

            <g aria-hidden>
              {arcs.map((a) => {
                const active = activeId === a.id;
                const weight = a.cluster.sockets.length / maxCount;
                return (
                  <path
                    key={a.id}
                    d={a.d}
                    className="map-arc"
                    fill="none"
                    stroke="var(--signal)"
                    strokeWidth={active ? 1.6 : 1}
                    strokeOpacity={activeId && !active ? 0.08 : active ? 0.85 : 0.18 + weight * 0.32}
                    strokeLinecap="round"
                  />
                );
              })}
            </g>

            {homePoint && home && (
              <g transform={`translate(${homePoint[0]} ${homePoint[1]})`} aria-label={`Home: ${home.label}`}>
                <circle r={9} fill="none" stroke="var(--fg)" strokeOpacity={0.25} strokeWidth={1} />
                <circle r={3.5} fill="var(--fg)" stroke="var(--sunken)" strokeWidth={2} />
              </g>
            )}

            {clusters.map((c) => {
              const r = markerRadius(c.sockets.length);
              const active = activeId === c.id;
              const muted = activeId !== null && !active;
              const busy = c.bps > 0;
              const pulse = busy ? Math.max(1.2, 2.8 - Math.log10(c.bps + 1) * 0.3) : 3.2;
              return (
                <g
                  key={c.id}
                  transform={`translate(${c.x} ${c.y})`}
                  role="button"
                  tabIndex={0}
                  aria-label={`${c.label}: ${pluralize(c.sockets.length, "connection")}`}
                  aria-pressed={selectedClusterId === c.id}
                  className="cursor-pointer outline-none"
                  onPointerEnter={() => setHovered({ cluster: c })}
                  onFocus={() => setHovered({ cluster: c })}
                  onBlur={() => setHovered(null)}
                  onClick={(event) => {
                    event.stopPropagation();
                    onSelectCluster(selectedClusterId === c.id ? null : selectionOf(c));
                  }}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      onSelectCluster(selectedClusterId === c.id ? null : selectionOf(c));
                    }
                  }}
                  opacity={muted ? 0.35 : 1}
                >
                  {!reduced && (
                    <circle
                      r={r}
                      className="map-pulse"
                      fill="var(--signal)"
                      style={{ ["--pulse" as string]: `${pulse}s`, ["--delay" as string]: `${(hash(c.id) % 2000) / 1000}s` }}
                    />
                  )}
                  <circle r={Math.max(14, r + 6)} fill="transparent" />
                  <circle r={r} fill="var(--signal)" fillOpacity={0.92} stroke="var(--sunken)" strokeWidth={2} />
                  {active && <circle r={r + 4} fill="none" stroke="var(--fg)" strokeWidth={1.2} />}
                </g>
              );
            })}
          </svg>

          {!reduced && !dimmed && (
            <div aria-hidden className="pointer-events-none absolute inset-0">
              {packets.map((a) => {
                const len = Math.hypot(a.cluster.x - (homePoint?.[0] ?? a.cluster.x), a.cluster.y - (homePoint?.[1] ?? a.cluster.y));
                const duration = 1.4 + len / 380;
                const muted = activeId !== null && activeId !== a.id;
                return (
                  <span
                    key={a.id}
                    className="map-packet"
                    style={{
                      offsetPath: `path("${a.d}")`,
                      ["--duration" as string]: `${duration.toFixed(2)}s`,
                      ["--delay" as string]: `${((hash(a.id) % 1000) / 1000) * duration}s`,
                      visibility: muted ? "hidden" : undefined,
                    }}
                  />
                );
              })}
            </div>
          )}

          {hovered && !dimmed && <ClusterTooltip cluster={hovered.cluster} width={width} hostnames={hostnames} />}
        </>
      )}
    </div>
  );
}
