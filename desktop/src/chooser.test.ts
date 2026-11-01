import { test, expect } from "bun:test";
import { watchChoosers } from "./chooser";

test("chooser polling recovers from a failed poll and aborts during backoff", async () => {
  let polls = 0;
  const server = Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    fetch(request) {
      expect(request.headers.get("cookie")).toBe(
        `boxbox_desktop=${"a".repeat(64)}`,
      );
      polls++;
      if (polls === 1) return new Response("temporary", { status: 503 });
      if (polls === 2) return new Response("invalid JSON");
      if (polls === 3) return Response.json([{ id: "b".repeat(32) }]);
      return new Response("temporary", { status: 503 });
    },
  });
  const controller = new AbortController();
  const updates: string[][] = [];
  const task = watchChoosers(
    `${server.url}_desktop?token=${"a".repeat(64)}`,
    controller.signal,
    (ids) => updates.push(ids),
  );
  try {
    const deadline = Date.now() + 4000;
    while (polls < 4 && Date.now() < deadline) await Bun.sleep(10);
    expect(polls).toBe(4);
    expect(updates).toEqual([["b".repeat(32)]]);
    controller.abort();
    await task;
  } finally {
    controller.abort();
    server.stop(true);
  }
});

test("invalid chooser bootstrap fails before polling", async () => {
  await expect(
    watchChoosers(
      "http://example.com/_desktop?token=bad",
      new AbortController().signal,
      () => {},
    ),
  ).rejects.toThrow("Invalid chooser bootstrap");
});
