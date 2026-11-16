// Counts the site shows from its own cached endpoints (downloads, stars).
// Each endpoint answers { total }.

/** What an endpoint returns; anything else is treated as no answer. */
export function parseCountPayload(payload: unknown): number | null {
  if (typeof payload !== "object" || payload === null || !("total" in payload)) return null;
  const { total } = payload;
  return typeof total === "number" && Number.isSafeInteger(total) && total >= 0 ? total : null;
}

/** 21287 reads as "21k"; rounding down never claims a number that was not reached. */
export function formatCount(total: number): string {
  if (total < 1000) return String(total);
  if (total < 1_000_000) return `${Math.floor(total / 1000)}k`;
  return `${(Math.floor(total / 100_000) / 10).toString()}M`;
}
