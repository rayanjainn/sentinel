import type { CleanupCategory } from "../../bindings/CleanupCategory";

export const CATEGORY_LABEL: Record<CleanupCategory, string> = {
  appCache: "App caches",
  logs: "Logs",
  trash: "Trash",
  oldDownloads: "Old downloads",
  buildArtifacts: "Build artifacts",
  packageManagerCache: "Package manager caches",
  developerCache: "Developer caches",
};

export const CATEGORY_ORDER: CleanupCategory[] = [
  "appCache",
  "developerCache",
  "packageManagerCache",
  "buildArtifacts",
  "oldDownloads",
  "logs",
  "trash",
];
