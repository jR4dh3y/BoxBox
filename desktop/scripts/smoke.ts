import { strict as assert } from "node:assert";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { startServer } from "../src/server";
import { request } from "node:http";
import { randomBytes } from "node:crypto";

const data = await mkdtemp(join(tmpdir(), "boxbox-desktop-smoke-"));
const server = await startServer(
  resolve(import.meta.dir, "../resources/boxbox-server"),
  data,
);
const origin = new URL(server.url).origin;
try {
  assert.equal((await fetch(origin + "/api/v1/files/")).status, 401);
  const bootstrap = await fetch(server.url, { redirect: "manual" });
  assert.equal(bootstrap.status, 303);
  const cookie = bootstrap.headers.get("set-cookie")?.split(";")[0];
  assert.ok(cookie);
  const headers = { Cookie: cookie };
  const page = await fetch(origin, { headers });
  assert.equal(page.status, 200);
  assert.match(await page.text(), /name="boxbox-desktop"/);
  assert.equal(
    (await fetch(origin + "/api/v1/files/", { headers })).status,
    200,
  );
  assert.equal(
    (
      await fetch(origin + "/api/v1/desktop/open", {
        method: "POST",
        headers: { ...headers, "Content-Type": "application/json" },
        body: JSON.stringify({ path: "outside/file" }),
      })
    ).status,
    403,
  );
  assert.equal(
    (
      await fetch(origin + "/api/v1/files/", {
        headers: { ...headers, Origin: "https://other.example" },
      })
    ).status,
    403,
  );
  await new Promise<void>((resolve, reject) => {
    const upgrade = request(origin + "/api/v1/ws", {
      headers: {
        ...headers,
        Origin: origin,
        Connection: "Upgrade",
        Upgrade: "websocket",
        "Sec-WebSocket-Version": "13",
        "Sec-WebSocket-Key": randomBytes(16).toString("base64"),
      },
    });
    upgrade.setTimeout(5000, () =>
      upgrade.destroy(new Error("WebSocket timeout")),
    );
    upgrade.on("upgrade", (response, socket) => {
      socket.destroy();
      assert.equal(response.statusCode, 101);
      resolve();
    });
    upgrade.on("response", () => {
      upgrade.destroy();
      reject(new Error("WebSocket upgrade rejected"));
    });
    upgrade.on("error", reject);
    upgrade.end();
  });
} finally {
  await server.stop();
  await rm(data, { recursive: true, force: true });
}
await assert.rejects(
  fetch(origin + "/health", { signal: AbortSignal.timeout(1000) }),
);
console.log(
  "Desktop smoke passed: private bootstrap, embedded UI, roots, confinement, Origin, WebSocket, shutdown.",
);
