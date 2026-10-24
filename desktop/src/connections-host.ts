import { browseURL, type LaunchTarget } from "./launch";

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
