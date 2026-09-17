import type { ResourceSample } from "../../bindings/ResourceSample";
import { storeSource } from "../../components/charts/LiveChart";
import { useResources } from "../../stores/resources";

/** Shared chart source over the resource sample ring. */
export const resourceSource = storeSource(useResources, (s) => s.samples);

export const sampleTime = (s: ResourceSample) => s.tsMs;
