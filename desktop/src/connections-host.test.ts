import { describe, expect, test } from "bun:test";
import { isConnectionsHostMessage, localBootstrapURL } from "./connections-host";

describe("desktop connection entry points", () => {
  test("accepts only the exact toolbar message from a webview event", () => {
    expect(
      isConnectionsHostMessage({ data: { detail: { type: "boxbox:connections" } } }),
    ).toBe(true);
    expect(
      isConnectionsHostMessage({
        data: { detail: JSON.stringify({ type: "boxbox:connections" }) },
      }),
    ).toBe(true);
    for (const event of [
      undefined,
      { detail: { type: "boxbox:connections" } },
      { data: { detail: "not json" } },
      { data: { detail: { type: "boxbox:other" } } },
      { data: { detail: { type: "boxbox:connections", url: "https://example.test" } } },
    ]) {
      expect(isConnectionsHostMessage(event)).toBe(false);
    }
  });

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
