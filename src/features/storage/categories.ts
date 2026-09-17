import type { CleanupCategory } from "../../bindings/CleanupCategory";
import type { GlossaryId } from "../../lib/glossary";

export const CATEGORY_LABEL: Record<CleanupCategory, string> = {
  appCache: "App caches",
  logs: "Logs",
  trash: "Trash",
  oldDownloads: "Old downloads",
  buildArtifacts: "Build artifacts",
  packageManagerCache: "Package manager caches",
  developerCache: "Developer caches",
};

/** The glossary entry explaining each cleanup category, read from the same module as every
 * other tooltip so the category header and the searchable Settings list never drift apart. */
export const CATEGORY_GLOSSARY: Record<CleanupCategory, GlossaryId> = {
  appCache: "cleanupAppCache",
  logs: "cleanupLogs",
  trash: "cleanupTrash",
  oldDownloads: "cleanupOldDownloads",
  buildArtifacts: "cleanupBuildArtifacts",
  packageManagerCache: "cleanupPackageManagerCache",
  developerCache: "cleanupDeveloperCache",
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
