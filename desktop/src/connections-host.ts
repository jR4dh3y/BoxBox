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

/** The backend owns profile persistence and device keys for every launch mode. */
export async function remoteLaunchTarget(
  bootstrapURL: string,
  remoteURL: string,
): Promise<LaunchTarget> {
  const bootstrap = new URL(bootstrapURL);
  const token = bootstrap.searchParams.get("token");
  if (
    bootstrap.protocol !== "http:" ||
    bootstrap.hostname !== "127.0.0.1" ||
    !token ||
    !/^[a-f0-9]{64}$/.test(token)
  ) {
    throw new Error("Invalid local desktop bootstrap");
  }
  const response = await fetch(new URL("/api/v1/desktop/servers/open", bootstrap), {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Cookie: `boxbox_desktop=${token}`,
    },
    body: JSON.stringify({ url: remoteURL }),
    redirect: "error",
    signal: AbortSignal.timeout(10_000),
  });
  if (!response.ok) {
    throw new Error(
      "Could not open the saved BoxBox server. Use an HTTPS deployment URL without a query or fragment.",
    );
  }
  const profile: unknown = await response.json();
  if (
    typeof profile !== "object" ||
    profile === null ||
    !("id" in profile) ||
    typeof profile.id !== "string" ||
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(profile.id)
  ) {
    throw new Error("Invalid saved server response");
  }
  return { path: `@remote/${profile.id}` };
}
