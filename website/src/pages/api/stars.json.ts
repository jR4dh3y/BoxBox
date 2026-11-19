import type { APIRoute } from "astro";
import { getStars } from "../../lib/stars";

export const prerender = false;

export const GET: APIRoute = async () => {
  const total = await getStars();
  const headers = {
    "Content-Type": "application/json",
    "Cache-Control": "public, s-maxage=3600, stale-while-revalidate=3600",
  };
  if (total === null) {
    return new Response(JSON.stringify({ error: "star count unavailable" }), { status: 503, headers });
  }
  return new Response(JSON.stringify({ total }), { status: 200, headers });
};
