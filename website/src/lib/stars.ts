// How many stars the repository has, for the GitHub link in the nav.

const REPO_API = "https://api.github.com/repos/jR4dh3y/BoxBox";
export const CACHE_TTL_MS = 60 * 60 * 1000;
let cache: { total: number; fetchedAt: number } | null = null;

function parseStars(payload: unknown): number | null {
  if (typeof payload !== "object" || payload === null || !("stargazers_count" in payload)) return null;
  const { stargazers_count: total } = payload;
  return typeof total === "number" && Number.isSafeInteger(total) && total >= 0 ? total : null;
}

/** The last good number keeps being served if GitHub is slow or rate limits. */
export async function getStars(): Promise<number | null> {
  const now = Date.now();
  if (cache && now - cache.fetchedAt < CACHE_TTL_MS) return cache.total;
  try {
    const response = await fetch(REPO_API, {
      headers: {
        Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "BoxBox-website",
      },
      signal: AbortSignal.timeout(10000),
    });
    if (!response.ok) throw new Error("Repository lookup failed");
    const total = parseStars(await response.json());
    if (total === null) throw new Error("Repository has no star count");
    cache = { total, fetchedAt: now };
    return total;
  } catch {
    return cache?.total ?? null;
  }
}
