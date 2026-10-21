import { browseURL, type LaunchTarget } from "./launch";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Accepts only the local browse toolbar's exact manager-open message. */
export function isConnectionsHostMessage(event: unknown): boolean {
  if (!isRecord(event) || !isRecord(event["data"])) return false;
  const detail = event["data"]["detail"];
  let payload: unknown = detail;
  if (typeof detail === "string") {
    try {
      payload = JSON.parse(detail);
    } catch {
      return false;
    }
  }
  return (
    isRecord(payload) &&
    Object.keys(payload).length === 1 &&
    payload["type"] === "boxbox:connections"
  );
}

/** Keeps the desktop token bootstrap first while asking it to open a local target. */
export function localBootstrapURL(
  serverURL: string,
  target?: LaunchTarget,
): string {
  const bootstrap = new URL(serverURL);
  if (!target) return bootstrap.href;
  const next = new URL(browseURL(serverURL, target));
  if (next.pathname !== "/browse") {
    throw new Error("The initial desktop target must open the browse page");
  }
  bootstrap.searchParams.set("next", `${next.pathname}${next.search}`);
  return bootstrap.href;
}
