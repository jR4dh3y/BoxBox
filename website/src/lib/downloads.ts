// How many times the BoxBox image has been pulled. GitHub has no public API
// for container pull counts, so the number is read from the package page.

const PACKAGE_PAGE = "https://github.com/jR4dh3y/BoxBox/pkgs/container/boxbox";
export const CACHE_TTL_MS = 60 * 60 * 1000;
let cache: { total: number; fetchedAt: number } | null = null;

/** The exact total sits in the title of the figure under "Total downloads". */
export function parseTotalDownloads(html: string): number | null {
  const match = /Total downloads<\/span>\s*<h3 title="(\d{1,12})"/.exec(html);
  return match?.[1] ? Number(match[1]) : null;
}

/** What the API returns; anything else is treated as no answer. */
export function parseDownloadsPayload(payload: unknown): number | null {
  if (typeof payload !== "object" || payload === null || !("total" in payload)) return null;
  const { total } = payload;
  return typeof total === "number" && Number.isSafeInteger(total) && total >= 0 ? total : null;
}

/** 21287 reads as "21k"; rounding down never claims a download that did not happen. */
export function formatDownloads(total: number): string {
  if (total < 1000) return String(total);
  if (total < 1_000_000) return `${Math.floor(total / 1000)}k`;
  return `${(Math.floor(total / 100_000) / 10).toString()}M`;
}

/** The last good number keeps being served if GitHub is slow or changes its page. */
export async function getTotalDownloads(): Promise<number | null> {
  const now = Date.now();
  if (cache && now - cache.fetchedAt < CACHE_TTL_MS) return cache.total;
  try {
    const response = await fetch(PACKAGE_PAGE, {
      headers: { "User-Agent": "BoxBox-website" },
      signal: AbortSignal.timeout(10000),
    });
    if (!response.ok) throw new Error("Package page lookup failed");
    const total = parseTotalDownloads(await response.text());
    if (total === null) throw new Error("Package page has no download total");
    cache = { total, fetchedAt: now };
    return total;
  } catch {
    return cache?.total ?? null;
  }
}
