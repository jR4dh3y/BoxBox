import type { APIRoute } from "astro";
import { getLatestRelease } from "../../lib/releases";

export const prerender = false;

export const GET: APIRoute = async () => {
  const release = await getLatestRelease();

  const headers = {
    "Content-Type": "application/json",
    // Allow the banner to read it from a different origin (e.g. the frontend).
    "Access-Control-Allow-Origin": "*",
    // Tell the browser it may cache the response for a short time; the server
    // cache is what actually protects the GitHub rate limit.
    "Cache-Control": "public, s-maxage=3600, stale-while-revalidate=3600",
  };

  if (!release) {
    return new Response(JSON.stringify({ error: "no release available" }), {
      status: 503,
      headers,
    });
  }

  return new Response(JSON.stringify(release), { status: 200, headers });
};