// Server-side helper for fetching the latest GitHub release with an in-memory
// cache. This keeps the GitHub API rate limit low: the endpoint only hits
// GitHub once per TTL window per server instance instead of on every request.

export interface LatestRelease {
  tagName: string;
  name: string;
  publishedAt: string;
  htmlUrl: string;
  summary: string;
}

interface CacheEntry {
  data: LatestRelease;
  fetchedAt: number;
}

const GITHUB_REPOS_OWNER = "jR4dh3y";
const GITHUB_REPOS_NAME = "BoxBox";
const GITHUB_API_URL = `https://api.github.com/repos/${GITHUB_REPOS_OWNER}/${GITHUB_REPOS_NAME}/releases/latest`;

// How long a fetched release is considered fresh before we re-fetch from GitHub.
export const CACHE_TTL_MS = 60 * 60 * 1000; // 1 hour

let cache: CacheEntry | null = null;

function releaseSummary(body: string | null | undefined): string {
  const text = (body ?? "").trim();
  if (!text) return "";

  // Grab the first paragraph, which on BoxBox releases is the overview line.
  const firstParagraph = text.split(/\r?\n\r?\n/)[0]?.trim() ?? "";
  return firstParagraph || (text.split(/\r?\n/)[0]?.trim() ?? "");
}

function parseRelease(payload: Record<string, unknown>): LatestRelease {
  return {
    tagName: String(payload.tag_name ?? ""),
    name: String(payload.name ?? ""),
    publishedAt: String(payload.published_at ?? ""),
    htmlUrl: String(payload.html_url ?? ""),
    summary: releaseSummary((payload.body as string | null | undefined) ?? ""),
  };
}

/**
 * Returns the latest release, using the in-memory cache when it is still fresh.
 * Falls back to stale cached data if a refresh request to GitHub fails, so the
 * landing page keeps working even when GitHub is unreachable.
 */
export async function getLatestRelease(): Promise<LatestRelease | null> {
  const now = Date.now();

  if (cache !== null && now - cache.fetchedAt < CACHE_TTL_MS) {
    return cache.data;
  }

  try {
    const response = await fetch(GITHUB_API_URL, {
      headers: {
        Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "BoxBox-website",
      },
      // Let the serverless function wait for a response rather than timing out
      // the first warm request immediately.
      signal: AbortSignal.timeout(10_000),
    });

    if (!response.ok) {
      throw new Error(`GitHub API responded with ${response.status}`);
    }

    const payload = (await response.json()) as Record<string, unknown>;
    const data = parseRelease(payload);
    cache = { data, fetchedAt: now };
    return data;
  } catch {
    // Return stale data if we have it; otherwise the caller can hide the banner.
    return cache?.data ?? null;
  }
}