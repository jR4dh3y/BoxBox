import { describe, expect, test } from "bun:test";
import { localBootstrapURL } from "./connections-host";

describe("desktop connection entry points", () => {
  test("keeps the local token bootstrap while adding the validated browse target", () => {
    const url = new URL(
      localBootstrapURL("http://127.0.0.1:4567/_desktop?token=local-token", {
        path: "home/Pictures",
        select: "home/Pictures/cover.png",
      }),
    );

    expect(url.pathname).toBe("/_desktop");
    expect(url.searchParams.get("token")).toBe("local-token");
    expect(url.searchParams.get("next")).toBe(
      "/browse?path=home%2FPictures&select=home%2FPictures%2Fcover.png",
    );
  });
});

test("remote launch uses the private local gateway and retains the bootstrap", async () => {
  const { remoteLaunchTarget } = await import("./connections-host");
  const token = "a".repeat(64);
  const id = "12345678-1234-4234-8234-123456789abc";
  const requests: string[] = [];
  const local = Bun.serve({
    hostname: "127.0.0.1",
    port: 0,
    async fetch(request) {
      expect(new URL(request.url).pathname).toBe("/api/v1/desktop/servers/open");
      expect(request.method).toBe("POST");
      expect(request.headers.get("cookie")).toBe(`boxbox_desktop=${token}`);
      expect(request.headers.get("authorization")).toBeNull();
      const body: unknown = await request.json();
      expect(body).toEqual({ url: "https://deployment.example/box" });
      requests.push(request.url);
      return Response.json({ id, name: "Saved deployment", icon: "cloud", expiresAt: "0001-01-01T00:00:00Z" });
    },
  });
  try {
    const bootstrap = `${local.url.origin}/_desktop?token=${token}`;
    const target = await remoteLaunchTarget(bootstrap, "https://deployment.example/box");
    expect(target).toEqual({ path: `@remote/${id}` });
    const url = new URL(localBootstrapURL(bootstrap, target));
    expect(url.origin).toBe(local.url.origin);
    expect(url.pathname).toBe("/_desktop");
    expect(url.searchParams.get("next")).toBe(`/browse?path=%40remote%2F${id}`);
    expect(requests).toHaveLength(1);
  } finally {
    await local.stop(true);
  }
});

test("remote launch rejects malformed profile responses and bootstrap destinations", async () => {
  const { remoteLaunchTarget } = await import("./connections-host");
  const local = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: () => Response.json({ id: "../../outside" }) });
  try {
    await expect(remoteLaunchTarget(`${local.url.origin}/_desktop?token=${"b".repeat(64)}`, "https://deployment.example")).rejects.toThrow("Invalid saved server response");
    await expect(remoteLaunchTarget(`https://outside.example/_desktop?token=${"b".repeat(64)}`, "https://deployment.example")).rejects.toThrow("Invalid local desktop bootstrap");
  } finally {
    await local.stop(true);
  }
});
