export const REPO = "rayanjainn/sentinel";
export const REPO_URL = `https://github.com/${REPO}`;
/** Always resolves to the newest published (non-draft, non-prerelease) release page. */
export const LATEST_RELEASE_URL = `${REPO_URL}/releases/latest`;
export const LATEST_RELEASE_API = `https://api.github.com/repos/${REPO}/releases/latest`;

/**
 * Prefixes a site-relative path with the configured `base` (`/` on the custom domain).
 * `href("docs/install/")` -> `/docs/install/`, `href("#download")` -> `/#download`.
 */
export function href(path = ""): string {
  const base = import.meta.env.BASE_URL.replace(/\/+$/, "");
  return `${base}/${path.replace(/^\/+/, "")}`;
}
