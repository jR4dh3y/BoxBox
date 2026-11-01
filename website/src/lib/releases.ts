export interface DesktopDownload {
  fileName: string;
  url: string;
  size: number;
}

export interface LatestRelease {
  tagName: string;
  name: string;
  publishedAt: string;
  htmlUrl: string;
  summary: string;
  desktop: DesktopDownload | null;
  checksumsUrl: string | null;
}

const repository = "https://github.com/jR4dh3y/BoxBox";
export const CACHE_TTL_MS = 60 * 60 * 1000;
let cache: { data: LatestRelease; fetchedAt: number } | null = null;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** A release without the desktop asset must never produce a desktop download link. */
export function parseRelease(payload: unknown): LatestRelease | null {
  if (
    !isRecord(payload) ||
    typeof payload.tag_name !== "string" ||
    !/^v\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$/.test(payload.tag_name)
  )
    return null;
  const tagName = payload.tag_name;
  const prefix = `${repository}/releases/download/${tagName}/`;
  const assets = Array.isArray(payload.assets)
    ? payload.assets.filter(isRecord)
    : [];
  const asset = (name: string) =>
    assets.find(
      (item) =>
        item.name === name && item.browser_download_url === prefix + name,
    );
  const fileName = `boxbox-desktop-${tagName}-linux-amd64.tar.gz`;
  const desktop = asset(fileName);
  const body = typeof payload.body === "string" ? payload.body.trim() : "";
  return {
    tagName,
    name:
      typeof payload.name === "string" && payload.name ? payload.name : tagName,
    publishedAt:
      typeof payload.published_at === "string" ? payload.published_at : "",
    htmlUrl: `${repository}/releases/tag/${tagName}`,
    summary: body.split(/\r?\n\r?\n/)[0]?.trim() ?? "",
    desktop:
      desktop &&
      typeof desktop.size === "number" &&
      Number.isSafeInteger(desktop.size) &&
      desktop.size > 0
        ? { fileName, url: prefix + fileName, size: desktop.size }
        : null,
    checksumsUrl: asset("SHA256SUMS") ? prefix + "SHA256SUMS" : null,
  };
}

/** Keep public release discovery available during transient GitHub failures. */
export async function getLatestRelease(): Promise<LatestRelease | null> {
  const now = Date.now();
  if (cache && now - cache.fetchedAt < CACHE_TTL_MS) return cache.data;
  try {
    const response = await fetch(
      "https://api.github.com/repos/jR4dh3y/BoxBox/releases/latest",
      {
        headers: {
          Accept: "application/vnd.github+json",
          "X-GitHub-Api-Version": "2022-11-28",
          "User-Agent": "BoxBox-website",
        },
        signal: AbortSignal.timeout(10000),
      },
    );
    if (!response.ok) throw new Error("Release lookup failed");
    const data = parseRelease(await response.json());
    if (!data) throw new Error("Invalid release metadata");
    cache = { data, fetchedAt: now };
    return data;
  } catch {
    return cache?.data ?? null;
  }
}
