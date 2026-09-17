export type Os = "macos" | "windows" | "linux";
export type Arch = "arm64" | "x64";

export const OS_LABEL: Record<Os, string> = {
  macos: "macOS",
  windows: "Windows",
  linux: "Linux",
};

export interface DetectedPlatform {
  /** Desktop OS Sentinel ships for, or null for phones, tablets, ChromeOS and unknown browsers. */
  os: Os | null;
  /** CPU architecture when the browser reveals it. Safari and Firefox never do. */
  arch: Arch | null;
}

// User-Agent Client Hints are Chromium-only and not in TypeScript's DOM lib.
interface UADataValues {
  architecture?: string;
}

interface NavigatorUAData {
  platform: string;
  getHighEntropyValues?: (hints: string[]) => Promise<UADataValues>;
}

export interface NavigatorLike {
  userAgent: string;
  maxTouchPoints?: number;
  userAgentData?: NavigatorUAData;
}

/** Client Hints platform names that are not desktop systems Sentinel supports. */
const UNSUPPORTED_PLATFORMS = new Set(["android", "chrome os", "chromeos", "ios"]);

/**
 * Maps `navigator.userAgentData.platform` (preferred) or the user-agent string to an OS.
 * Exported separately so it can be exercised without a browser.
 */
export function detectOs(platform: string, userAgent: string, maxTouchPoints = 0): Os | null {
  const hint = platform.trim().toLowerCase();
  if (hint === "macos") return "macos";
  if (hint === "windows") return "windows";
  if (hint === "linux") return "linux";
  if (UNSUPPORTED_PLATFORMS.has(hint)) return null;

  if (/Android|iPhone|iPad|iPod|CrOS/i.test(userAgent)) return null;
  // iPadOS Safari sends a desktop Mac user agent; touch support gives it away.
  if (/Macintosh|Mac OS X/i.test(userAgent)) return maxTouchPoints > 1 ? null : "macos";
  if (/Windows/i.test(userAgent)) return "windows";
  if (/Linux|X11/i.test(userAgent)) return "linux";
  return null;
}

export async function detectPlatform(nav: NavigatorLike = navigator): Promise<DetectedPlatform> {
  const uaData = nav.userAgentData;
  const os = detectOs(uaData?.platform ?? "", nav.userAgent, nav.maxTouchPoints ?? 0);
  let arch: Arch | null = null;

  if (os && uaData?.getHighEntropyValues) {
    try {
      const { architecture } = await uaData.getHighEntropyValues(["architecture"]);
      if (architecture === "arm") arch = "arm64";
      else if (architecture === "x86") arch = "x64";
    } catch {
      // The browser may refuse high-entropy hints; architecture stays unknown.
    }
  }

  if (!arch && os !== "macos" && /aarch64|arm64/i.test(nav.userAgent)) arch = "arm64";
  return { os, arch };
}
