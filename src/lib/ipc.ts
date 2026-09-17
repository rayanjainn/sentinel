// Typed bridge to the Rust backend. Mirrors docs/CONTRACT.md; every invoke and event goes through here.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { Action } from "../bindings/Action";
import type { ActionDecision } from "../bindings/ActionDecision";
import type { ActionOutcome } from "../bindings/ActionOutcome";
import type { ActionPreview } from "../bindings/ActionPreview";
import type { AgentSettings } from "../bindings/AgentSettings";
import type { AgentStreamPayload } from "../bindings/AgentStreamPayload";
import type { AuditEntry } from "../bindings/AuditEntry";
import type { AuditQuery } from "../bindings/AuditQuery";
import type { DuplicateProgress } from "../bindings/DuplicateProgress";
import type { DuplicateReport } from "../bindings/DuplicateReport";
import type { ErrorPayload } from "../bindings/ErrorPayload";
import type { FirewallRule } from "../bindings/FirewallRule";
import type { FirewallStatus } from "../bindings/FirewallStatus";
import type { GeoDbStatus } from "../bindings/GeoDbStatus";
import type { HomeLocation } from "../bindings/HomeLocation";
import type { HomeLocationInput } from "../bindings/HomeLocationInput";
import type { HostResolved } from "../bindings/HostResolved";
import type { KeyStatus } from "../bindings/KeyStatus";
import type { ModelInfo } from "../bindings/ModelInfo";
import type { NetworkSnapshot } from "../bindings/NetworkSnapshot";
import type { OllamaStatus } from "../bindings/OllamaStatus";
import type { PermissionKind } from "../bindings/PermissionKind";
import type { PermissionStatus } from "../bindings/PermissionStatus";
import type { Plan } from "../bindings/Plan";
import type { ProcessDetail } from "../bindings/ProcessDetail";
import type { ProcessHistoryPoint } from "../bindings/ProcessHistoryPoint";
import type { ProcessSnapshot } from "../bindings/ProcessSnapshot";
import type { ProviderDescriptor } from "../bindings/ProviderDescriptor";
import type { ProviderId } from "../bindings/ProviderId";
import type { ProviderStatus } from "../bindings/ProviderStatus";
import type { ResourceSample } from "../bindings/ResourceSample";
import type { SamplingConfig } from "../bindings/SamplingConfig";
import type { ScanPartial } from "../bindings/ScanPartial";
import type { ScanProgress } from "../bindings/ScanProgress";
import type { ScanRequest } from "../bindings/ScanRequest";
import type { ScanSummary } from "../bindings/ScanSummary";
import type { SystemInfo } from "../bindings/SystemInfo";
import type { TranscriptItem } from "../bindings/TranscriptItem";
import type { TreeNode } from "../bindings/TreeNode";
import type { TreeQuery } from "../bindings/TreeQuery";
import type { VolumeInfo } from "../bindings/VolumeInfo";

export class IpcError extends Error {
  readonly payload: ErrorPayload;

  constructor(payload: ErrorPayload) {
    super(payload.message);
    this.name = "IpcError";
    this.payload = payload;
  }
}

function isErrorPayload(value: unknown): value is ErrorPayload {
  return (
    typeof value === "object" &&
    value !== null &&
    "code" in value &&
    "message" in value &&
    typeof (value as { message: unknown }).message === "string"
  );
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (raw) {
    if (isErrorPayload(raw)) throw new IpcError(raw);
    const detail = raw instanceof Error ? raw.message : String(raw);
    throw new IpcError({ code: "internal", detail, message: `internal error: ${detail}` });
  }
}

export const api = {
  // System / resources
  getSystemInfo: () => call<SystemInfo>("get_system_info"),
  getResourceHistory: (windowSecs: number) =>
    call<ResourceSample[]>("get_resource_history", { windowSecs }),
  setSampling: (config: SamplingConfig) => call<SamplingConfig>("set_sampling", { config }),
  getPermissionStatus: () => call<PermissionStatus>("get_permission_status"),
  openPermissionSettings: (kind: PermissionKind) => call<void>("open_permission_settings", { kind }),

  // Processes
  getProcessSnapshot: () => call<ProcessSnapshot>("get_process_snapshot"),
  getProcessDetail: (pid: number) => call<ProcessDetail>("get_process_detail", { pid }),
  getProcessHistory: (pid: number) => call<ProcessHistoryPoint[]>("get_process_history", { pid }),
  revealProcessExecutable: (pid: number) => call<void>("reveal_process_executable", { pid }),

  // Network
  getNetworkSnapshot: () => call<NetworkSnapshot>("get_network_snapshot"),
  getGeoDbStatus: () => call<GeoDbStatus>("get_geo_db_status"),
  downloadGeoDb: () => call<void>("download_geo_db"),
  getHomeLocation: () => call<HomeLocation>("get_home_location"),
  setHomeLocation: (location: HomeLocationInput | null) =>
    call<HomeLocation>("set_home_location", { location }),

  // Firewall
  getFirewallStatus: () => call<FirewallStatus>("get_firewall_status"),
  listFirewallRules: () => call<FirewallRule[]>("list_firewall_rules"),

  // Storage
  listVolumes: () => call<VolumeInfo[]>("list_volumes"),
  startScan: (request: ScanRequest) => call<string>("start_scan", { request }),
  cancelScan: (scanId: string) => call<void>("cancel_scan", { scanId }),
  getScanTree: (query: TreeQuery) => call<TreeNode>("get_scan_tree", { query }),
  getScanSummary: (scanId: string) => call<ScanSummary>("get_scan_summary", { scanId }),
  startDuplicateScan: (scanId: string, minSizeBytes: number) =>
    call<string>("start_duplicate_scan", { scanId, minSizeBytes }),
  cancelDuplicateScan: (jobId: string) => call<void>("cancel_duplicate_scan", { jobId }),
  getDuplicateReport: (jobId: string) => call<DuplicateReport>("get_duplicate_report", { jobId }),
  revealPath: (path: string) => call<void>("reveal_path", { path }),

  // Actions (shared by UI and agent) + audit
  prepareAction: (action: Action) => call<ActionPreview>("prepare_action", { action }),
  commitAction: (token: string) => call<ActionOutcome>("commit_action", { token }),
  rejectAction: (token: string) => call<void>("reject_action", { token }),
  getAuditLog: (query: AuditQuery) => call<AuditEntry[]>("get_audit_log", { query }),

  // Agent
  agent: {
    listProviders: () => call<ProviderDescriptor[]>("agent_list_providers"),
    getSettings: () => call<AgentSettings>("agent_get_settings"),
    updateSettings: (settings: AgentSettings) =>
      call<AgentSettings>("agent_update_settings", { settings }),
    providerStatus: () => call<ProviderStatus[]>("agent_provider_status"),
    setApiKey: (provider: ProviderId, key: string) =>
      call<KeyStatus>("agent_set_api_key", { provider, key }),
    deleteApiKey: (provider: ProviderId) => call<void>("agent_delete_api_key", { provider }),
    listModels: (provider: ProviderId) => call<ModelInfo[]>("agent_list_models", { provider }),
    ollamaStatus: () => call<OllamaStatus>("agent_ollama_status"),
    newConversation: () => call<string>("agent_new_conversation"),
    sendMessage: (conversationId: string, text: string) =>
      call<void>("agent_send_message", { conversationId, text }),
    cancel: (conversationId: string) => call<void>("agent_cancel", { conversationId }),
    getTranscript: (conversationId: string) =>
      call<TranscriptItem[]>("agent_get_transcript", { conversationId }),
    revisePlanAction: (planId: string, actionId: string, action: Action) =>
      call<Plan>("agent_revise_plan_action", { planId, actionId, action }),
    executePlan: (planId: string, decisions: ActionDecision[]) =>
      call<Plan>("agent_execute_plan", { planId, decisions }),
  },
} as const;

export interface EventMap {
  "sentinel:resources": ResourceSample;
  "sentinel:processes": ProcessSnapshot;
  "sentinel:network": NetworkSnapshot;
  "sentinel:host-resolved": HostResolved;
  "sentinel:scan-progress": ScanProgress;
  "sentinel:scan-partial": ScanPartial;
  "sentinel:scan-complete": ScanSummary;
  "sentinel:duplicate-progress": DuplicateProgress;
  "sentinel:duplicate-complete": DuplicateReport;
  "sentinel:geo-db": GeoDbStatus;
  "sentinel:agent": AgentStreamPayload;
}

export function on<K extends keyof EventMap>(
  event: K,
  handler: (payload: EventMap[K]) => void,
): Promise<UnlistenFn> {
  return listen<EventMap[K]>(event, (e) => handler(e.payload));
}
