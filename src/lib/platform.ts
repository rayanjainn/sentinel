// Client-side platform hints for chrome layout only (data comes from `get_system_info`).
export const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.userAgent);

export const modKey = isMac ? "⌘" : "Ctrl+";

export function isModEvent(event: KeyboardEvent): boolean {
  return isMac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}

export const fileManagerName = isMac
  ? "Finder"
  : typeof navigator !== "undefined" && /Windows/.test(navigator.userAgent)
    ? "Explorer"
    : "file manager";
