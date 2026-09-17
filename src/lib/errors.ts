// Turns backend errors into specific, actionable copy. Views render these through <ErrorState />.
import type { ErrorPayload } from "../bindings/ErrorPayload";
import type { PermissionKind } from "../bindings/PermissionKind";
import { IpcError } from "./ipc";

export interface ErrorDescription {
  code: ErrorPayload["code"];
  title: string;
  detail: string;
  /** Present when the fix is a system permission the app can deep-link to. */
  permission: PermissionKind | null;
  /** "Not available on this system": rendered as a quiet note rather than an alarm. */
  quiet: boolean;
}

function isPayload(value: unknown): value is ErrorPayload {
  return typeof value === "object" && value !== null && "code" in value && "message" in value;
}

export function toPayload(error: unknown): ErrorPayload {
  if (error instanceof IpcError) return error.payload;
  if (isPayload(error)) return error;
  const detail = error instanceof Error ? error.message : String(error);
  return { code: "internal", detail, message: detail };
}

function mentionsDiskAccess(text: string | null | undefined): boolean {
  return Boolean(text && /full disk access|tcc|privacy/i.test(text));
}

export function describeError(error: unknown, subject?: string): ErrorDescription {
  const p = toPayload(error);
  const base = { code: p.code, permission: null, quiet: false } as const;
  switch (p.code) {
    case "permissionDenied": {
      const wantsFda = mentionsDiskAccess(p.hint) || /read|scan|open files|file/i.test(p.operation);
      return {
        ...base,
        title: p.target ? `Permission denied: ${p.target}` : `Permission denied while trying to ${p.operation}`,
        detail: p.hint ?? p.message,
        permission: wantsFda ? "fullDiskAccess" : "administrator",
      };
    }
    case "processNotFound":
      return {
        ...base,
        title: `Process ${p.pid} is no longer running`,
        detail: "It exited before Sentinel could reach it. Nothing was changed.",
      };
    case "processChanged":
      return {
        ...base,
        title: `PID ${p.pid} now belongs to a different process`,
        detail: "The original process exited and its PID was reused, so the action was refused.",
      };
    case "pathNotFound":
      return { ...base, title: "Item no longer exists", detail: `${p.path} was moved or deleted.` };
    case "unavailable":
      if (/\bstill\b/i.test(p.reason)) {
        return {
          ...base,
          quiet: true,
          title: subject ? `${subject} is not ready yet` : "Not ready yet",
          detail: `Sentinel is ${p.reason.replace(/\.$/, "")}. Try again when it finishes.`,
        };
      }
      return {
        ...base,
        quiet: true,
        title: subject ? `${subject} is not available on this system` : "Not available on this system",
        detail: p.reason,
      };
    case "elevationDeclined":
      return {
        ...base,
        title: "Administrator authorization was declined",
        detail: `Sentinel needs administrator rights to ${p.operation}. Nothing was changed.`,
      };
    case "invalidInput":
      return { ...base, title: "Invalid input", detail: p.detail };
    case "actionTokenInvalid":
      return {
        ...base,
        title: "This confirmation expired",
        detail: "Previews are valid for 5 minutes and can be used once. Review the refreshed preview.",
      };
    case "cancelled":
      return { ...base, quiet: true, title: "Cancelled", detail: "The operation was cancelled." };
    case "network":
      return { ...base, title: "Network request failed", detail: p.detail };
    case "provider":
      return {
        ...base,
        title: `${p.provider} returned an error${p.status ? ` (${p.status})` : ""}`,
        detail: p.detail,
      };
    case "io":
      return {
        ...base,
        title: p.path ? `Could not access ${p.path}` : "File system error",
        detail: p.detail,
        permission: mentionsDiskAccess(p.detail) ? "fullDiskAccess" : null,
      };
    case "internal":
      return { ...base, title: subject ? `${subject} failed` : "Something went wrong", detail: p.detail };
  }
}

export function isCode(error: unknown, code: ErrorPayload["code"]): boolean {
  return error instanceof IpcError && error.payload.code === code;
}
