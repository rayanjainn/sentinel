// One name per action through the whole flow: the confirm button and the result share a verb.
import type { Action } from "../../bindings/Action";
import type { ActionPreview } from "../../bindings/ActionPreview";
import type { ActionRisk } from "../../bindings/ActionRisk";

export function confirmLabel(action: Action): string {
  switch (action.type) {
    case "terminateProcess":
      return "Quit";
    case "forceKillProcess":
      return "Force quit";
    case "setProcessPriority":
      return "Change priority";
    case "trashPaths":
      return "Move to Trash";
    case "movePaths":
      return "Move";
    case "addFirewallRule":
      return action.target.type === "remoteIp" ? "Block address" : "Block port";
    case "removeFirewallRule":
      return "Remove rule";
  }
}

export function doneLabel(action: Action): string {
  switch (action.type) {
    case "terminateProcess":
      return "Quit";
    case "forceKillProcess":
      return "Force quit";
    case "setProcessPriority":
      return "Priority changed";
    case "trashPaths":
      return "Moved to Trash";
    case "movePaths":
      return "Moved";
    case "addFirewallRule":
      return action.target.type === "remoteIp" ? "Address blocked" : "Port blocked";
    case "removeFirewallRule":
      return "Rule removed";
  }
}

export function riskLabel(risk: ActionRisk): string {
  switch (risk) {
    case "moderate":
      return "Moderate risk";
    case "high":
      return "High risk";
    case "critical":
      return "Critical: changes system network policy";
  }
}

/**
 * The exact text a user must type before a critical action can be confirmed. Uses the concrete
 * target (IP address, port, rule target) so the confirmation restates what will change.
 */
export function confirmationPhrase(preview: Pick<ActionPreview, "action" | "targets" | "title">): string {
  const { action } = preview;
  if (action.type === "addFirewallRule") {
    return action.target.type === "remoteIp" ? action.target.ip : String(action.target.port);
  }
  return preview.targets[0]?.label ?? preview.title;
}

export function isExpired(preview: Pick<ActionPreview, "expiresAtMs">, nowMs = Date.now()): boolean {
  return nowMs >= preview.expiresAtMs;
}

/** Normalizes whitespace so a trailing space typed by the user does not block confirmation. */
export function phraseMatches(expected: string, typed: string): boolean {
  return typed.trim().toLowerCase() === expected.trim().toLowerCase();
}
