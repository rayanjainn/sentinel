// Agent UI rules that do not depend on React: tool labels, plan review, and readiness checks.
import type { ActionDecision } from "../../bindings/ActionDecision";
import type { AgentSettings } from "../../bindings/AgentSettings";
import type { ErrorPayload } from "../../bindings/ErrorPayload";
import type { ModelInfo } from "../../bindings/ModelInfo";
import type { OllamaStatus } from "../../bindings/OllamaStatus";
import type { Plan } from "../../bindings/Plan";
import type { PlanAction } from "../../bindings/PlanAction";
import type { ProviderDescriptor } from "../../bindings/ProviderDescriptor";
import type { ProviderId } from "../../bindings/ProviderId";
import type { ProviderStatus } from "../../bindings/ProviderStatus";
import { confirmationPhrase, phraseMatches } from "../actions/labels";

export const EXAMPLE_REQUESTS = [
  "Free up 10 GB but don't touch anything I've used in the last 2 weeks",
  "Kill whatever's using port 5432",
  "What's eating my CPU right now, and can you fix it?",
] as const;

const TOOL_LABELS: Record<string, { running: string; done: string }> = {
  get_system_overview: { running: "Reading system overview", done: "Read system overview" },
  get_resource_usage: { running: "Reading CPU and memory", done: "Read CPU and memory" },
  list_processes: { running: "Listing processes", done: "Listed processes" },
  get_process_details: { running: "Inspecting process", done: "Inspected process" },
  list_network_sockets: { running: "Listing connections", done: "Listed connections" },
  find_port_owner: { running: "Finding port owner", done: "Found port owner" },
  list_volumes: { running: "Listing volumes", done: "Listed volumes" },
  scan_storage: { running: "Scanning storage", done: "Scanned storage" },
  get_storage_breakdown: { running: "Measuring folders", done: "Measured folders" },
  find_large_files: { running: "Finding large files", done: "Found large files" },
  get_cleanup_suggestions: { running: "Finding cleanup suggestions", done: "Found cleanup suggestions" },
  find_duplicate_files: { running: "Finding duplicates", done: "Found duplicates" },
  list_firewall_rules: { running: "Reading firewall rules", done: "Read firewall rules" },
  terminate_process: { running: "Proposing quit", done: "Proposed quit" },
  force_kill_process: { running: "Proposing force quit", done: "Proposed force quit" },
  set_process_priority: { running: "Proposing priority change", done: "Proposed priority change" },
  move_to_trash: { running: "Proposing Move to Trash", done: "Proposed Move to Trash" },
  move_paths: { running: "Proposing move", done: "Proposed move" },
  block_remote_ip: { running: "Proposing address block", done: "Proposed address block" },
  block_local_port: { running: "Proposing port block", done: "Proposed port block" },
  remove_firewall_rule: { running: "Proposing rule removal", done: "Proposed rule removal" },
};

export function toolLabel(name: string, finished: boolean): string {
  const label = TOOL_LABELS[name];
  if (!label) return name;
  return finished ? label.done : label.running;
}

// -- plan review --------------------------------------------------------------------------------

export type ReviewDecision = "approve" | "reject";

export interface PlanReview {
  decisions: Record<string, ReviewDecision>;
  /** Text typed to confirm critical actions, keyed by plan action id. */
  confirmations: Record<string, string>;
}

export const emptyReview: PlanReview = { decisions: {}, confirmations: {} };

export function needsTypedConfirmation(action: PlanAction): boolean {
  return action.preview.risk === "critical";
}

export function isConfirmed(action: PlanAction, review: PlanReview): boolean {
  if (!needsTypedConfirmation(action)) return true;
  return phraseMatches(confirmationPhrase(action.preview), review.confirmations[action.id] ?? "");
}

/** Approved and, for critical actions, confirmed by typing the target. */
export function isApproved(action: PlanAction, review: PlanReview): boolean {
  return review.decisions[action.id] === "approve" && isConfirmed(action, review);
}

export function approvedCount(plan: Plan, review: PlanReview): number {
  return plan.actions.filter((a) => a.state.state === "pending" && isApproved(a, review)).length;
}

/** Every pending action gets an explicit decision; anything not fully approved is rejected. */
export function decisionsFor(plan: Plan, review: PlanReview): ActionDecision[] {
  return plan.actions
    .filter((a) => a.state.state === "pending")
    .map((a) => ({ actionId: a.id, decision: isApproved(a, review) ? "approve" : "reject" }));
}

// -- readiness ----------------------------------------------------------------------------------

export type Readiness =
  | { state: "checking" }
  | { state: "ready" }
  | { state: "missingKey"; provider: ProviderId }
  | { state: "invalidKey"; provider: ProviderId; message: string }
  | { state: "ollamaMissing" }
  | { state: "ollamaStopped"; baseUrl: string }
  | { state: "modelMissing"; model: string }
  | { state: "noTools"; model: string };

export function selectedModel(
  settings: AgentSettings,
  provider: ProviderId,
  descriptors: ProviderDescriptor[] | null,
): string {
  const chosen = settings.selectedModels[provider];
  if (chosen) return chosen;
  const descriptor = descriptors?.find((d) => d.id === provider);
  return descriptor?.suggestedModels.find((m) => m.recommended)?.id ?? "";
}

/** Models offered for `provider`: installed Ollama models, the live list, or suggestions. */
export function modelChoices(
  provider: ProviderId,
  current: string,
  live: ModelInfo[] | undefined,
  installed: ModelInfo[] | null,
  descriptor: ProviderDescriptor | undefined,
): ModelInfo[] {
  const base =
    provider === "ollama" && installed && installed.length > 0 ? installed : (live ?? descriptor?.suggestedModels ?? []);
  if (!current || base.some((m) => m.id === current)) return base;
  return [{ id: current, displayName: current, supportsTools: true, contextWindow: null, recommended: false }, ...base];
}

export function readiness(input: {
  settings: AgentSettings | null;
  descriptors: ProviderDescriptor[] | null;
  statuses: ProviderStatus[] | null;
  ollama: OllamaStatus | null;
  models: ModelInfo[] | null;
}): Readiness {
  const { settings, descriptors, statuses, ollama, models } = input;
  if (!settings) return { state: "checking" };
  const provider = settings.activeProvider;
  const model = selectedModel(settings, provider, descriptors);
  if (provider === "ollama") {
    if (!ollama) return { state: "checking" };
    if (!ollama.installed) return { state: "ollamaMissing" };
    if (!ollama.running) return { state: "ollamaStopped", baseUrl: ollama.baseUrl };
    const installed = ollama.models.find((m) => m.id === model);
    if (!installed) return { state: "modelMissing", model };
    if (!installed.supportsTools) return { state: "noTools", model };
    return { state: "ready" };
  }
  const status = statuses?.find((s) => s.id === provider);
  if (!status) return { state: "checking" };
  if (status.key.state === "missing") return { state: "missingKey", provider };
  if (status.key.state === "invalid") return { state: "invalidKey", provider, message: status.key.message };
  const info = models?.find((m) => m.id === model);
  if (info && !info.supportsTools) return { state: "noTools", model };
  return { state: "ready" };
}

/** Maps a failed send to the readiness problem it reveals, when it is one. */
export function readinessFromError(error: ErrorPayload, settings: AgentSettings | null, model: string): Readiness | null {
  if (error.code === "unavailable") {
    if (error.feature.endsWith("API key") && settings) return { state: "missingKey", provider: settings.activeProvider };
    if (error.feature === "Ollama") {
      return { state: "ollamaStopped", baseUrl: settings?.ollamaBaseUrl ?? "http://127.0.0.1:11434" };
    }
    if (error.feature === "tool calling") return { state: "noTools", model };
  }
  if (error.code === "provider" && error.status === 404 && error.detail.includes("ollama pull")) {
    return { state: "modelMissing", model };
  }
  if (error.code === "provider" && (error.status === 401 || error.status === 403) && settings) {
    return { state: "invalidKey", provider: settings.activeProvider, message: error.detail };
  }
  return null;
}
