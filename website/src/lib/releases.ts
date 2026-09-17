// Explicit .ts extensions let Node's built-in test runner load this module directly.
import type { Os } from "./platform.ts";
import { LATEST_RELEASE_API, LATEST_RELEASE_URL } from "./site.ts";

export type AssetKey =
  | "macos-arm64"
  | "macos-x64"
  | "windows-x64-setup"
  | "windows-x64-msi"
  | "linux-x64-appimage"
  | "linux-x64-deb";

export interface AssetSpec {
  key: AssetKey;
  os: Os;
  /**
   * Everything after `Sentinel-<version>`. Must match `releaseAssetNamePattern` and the expected
   * asset list in .github/workflows/release.yml.
   */
  suffix: string;
  label: string;
  format: string;
}

export const ASSET_SPECS: readonly AssetSpec[] = [
  { key: "macos-arm64", os: "macos", suffix: "-macos-arm64.dmg", label: "Apple Silicon", format: "Disk image" },
  { key: "macos-x64", os: "macos", suffix: "-macos-x64.dmg", label: "Intel", format: "Disk image" },
  { key: "windows-x64-setup", os: "windows", suffix: "-windows-x64-setup.exe", label: "x64", format: "Installer" },
  { key: "windows-x64-msi", os: "windows", suffix: "-windows-x64.msi", label: "x64", format: "MSI package" },
  { key: "linux-x64-appimage", os: "linux", suffix: "-linux-x64.AppImage", label: "x64", format: "AppImage" },
  { key: "linux-x64-deb", os: "linux", suffix: "-linux-x64.deb", label: "x64", format: "Debian package" },
];

export const CHECKSUMS_FILE = "SHA256SUMS.txt";

export function assetFileName(version: string, spec: AssetSpec): string {
  return `Sentinel-${version}${spec.suffix}`;
}

export interface ReleaseAsset {
  name: string;
  url: string;
  size: number;
}

export interface LatestRelease {
  version: string;
  tag: string;
  pageUrl: string;
  publishedAt: string | null;
  assets: Partial<Record<AssetKey, ReleaseAsset>>;
  checksums: ReleaseAsset | null;
}

export type ReleaseLookup =
  | { status: "found"; release: LatestRelease }
  | { status: "none" }
  | { status: "error"; reason: string };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/** Turns a GitHub "latest release" API response into the assets the site links to. */
export function parseRelease(data: unknown): LatestRelease | null {
  if (!isRecord(data) || typeof data.tag_name !== "string") return null;

  const tag = data.tag_name;
  const version = tag.replace(/^v/, "");
  const byName = new Map<string, ReleaseAsset>();
  for (const asset of Array.isArray(data.assets) ? data.assets : []) {
    if (!isRecord(asset)) continue;
    const { name, browser_download_url: url, size } = asset;
    if (typeof name !== "string" || typeof url !== "string") continue;
    byName.set(name, { name, url, size: typeof size === "number" ? size : 0 });
  }

  const assets: Partial<Record<AssetKey, ReleaseAsset>> = {};
  for (const spec of ASSET_SPECS) {
    const match = byName.get(assetFileName(version, spec));
    if (match) assets[spec.key] = match;
  }

  return {
    version,
    tag,
    pageUrl: typeof data.html_url === "string" ? data.html_url : LATEST_RELEASE_URL,
    publishedAt: typeof data.published_at === "string" ? data.published_at : null,
    assets,
    checksums: byName.get(CHECKSUMS_FILE) ?? null,
  };
}

// Unauthenticated GitHub API calls are limited to 60 an hour per IP, so reuse a recent answer.
const CACHE_KEY = "sentinel-latest-release-v1";
const CACHE_TTL_MS = 10 * 60 * 1000;

function readCache(): ReleaseLookup | null {
  try {
    const raw = sessionStorage.getItem(CACHE_KEY);
    if (!raw) return null;
    const cached = JSON.parse(raw) as { savedAt: number; value: ReleaseLookup };
    return Date.now() - cached.savedAt < CACHE_TTL_MS ? cached.value : null;
  } catch {
    return null;
  }
}

function writeCache(value: ReleaseLookup) {
  try {
    sessionStorage.setItem(CACHE_KEY, JSON.stringify({ savedAt: Date.now(), value }));
  } catch {
    // Storage full or blocked: the next visit simply asks GitHub again.
  }
}

export async function lookupLatestRelease(fetchImpl: typeof fetch = fetch): Promise<ReleaseLookup> {
  const cached = readCache();
  if (cached) return cached;

  let response: Response;
  try {
    response = await fetchImpl(LATEST_RELEASE_API, {
      headers: { Accept: "application/vnd.github+json" },
      signal: AbortSignal.timeout(8000),
    });
  } catch {
    return { status: "error", reason: "GitHub could not be reached" };
  }

  // 404 means the repository has no published release yet.
  if (response.status === 404) {
    const none: ReleaseLookup = { status: "none" };
    writeCache(none);
    return none;
  }
  if (response.status === 403 || response.status === 429) {
    return { status: "error", reason: "your network has hit GitHub's hourly API limit" };
  }
  if (!response.ok) {
    return { status: "error", reason: `GitHub answered with status ${response.status}` };
  }

  let body: unknown;
  try {
    body = await response.json();
  } catch {
    return { status: "error", reason: "GitHub sent a response the page couldn't read" };
  }

  const release = parseRelease(body);
  if (!release) return { status: "error", reason: "GitHub sent a response the page couldn't read" };

  const found: ReleaseLookup = { status: "found", release };
  writeCache(found);
  return found;
}

export function formatBytes(bytes: number): string {
  if (bytes <= 0) return "";
  const megabytes = bytes / 1_000_000;
  return megabytes >= 100 ? `${Math.round(megabytes)} MB` : `${megabytes.toFixed(1)} MB`;
}
