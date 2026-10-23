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
