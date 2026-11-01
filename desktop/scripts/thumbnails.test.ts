import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { copyFile, mkdtemp, open, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { startServer, type StartedServer } from "../src/server";
import { buildNativeThumbnails } from "./build-native";
import { createThumbnailFixtures } from "./thumbnail-fixtures";

const repository = resolve(import.meta.dir, "../..");
let fixture: string;
let server: StartedServer;
let origin: string;
let cookie: string;

function jpegDimensions(data: Uint8Array): { width: number; height: number } {
  const bytes = Buffer.from(data);
  if (bytes[0] !== 0xff || bytes[1] !== 0xd8) throw new Error("Not a JPEG");
  let offset = 2;
  while (offset + 9 < bytes.byteLength) {
    if (bytes[offset] !== 0xff) throw new Error("Invalid JPEG marker");
    const marker = bytes[offset + 1];
    if (marker === 0xc0 || marker === 0xc1 || marker === 0xc2)
      return {
        height: bytes.readUInt16BE(offset + 5),
        width: bytes.readUInt16BE(offset + 7),
      };
    offset += 2 + bytes.readUInt16BE(offset + 2);
  }
  throw new Error("Missing JPEG dimensions");
}

async function preview(name: string, headers: Record<string, string> = {}) {
  const path = `root${join(fixture, name)}`;
  return fetch(
    `${origin}/api/v1/stream/thumbnail/${path}?width=224&height=144`,
    { headers: { Cookie: cookie, ...headers } },
  );
}

beforeAll(async () => {
  fixture = await mkdtemp(join(tmpdir(), "boxbox-thumbnails-"));
  await createThumbnailFixtures(fixture);
  await buildNativeThumbnails();
  const binary = join(fixture, "boxbox-server");
  const build = Bun.spawn(["go", "build", "-o", binary, "./cmd/server"], {
    cwd: join(repository, "backend"),
    stdout: "ignore",
    stderr: "inherit",
  });
  if ((await build.exited) !== 0)
    throw new Error("Thumbnail integration server build failed");
  // Use the same asset layout as the installed desktop bundle.
  const link = Bun.spawn([
    "ln",
    "-s",
    join(repository, "desktop/resources/native"),
    join(fixture, "native"),
  ]);
  if ((await link.exited) !== 0)
    throw new Error("Could not stage native thumbnail resources");
  server = await startServer(binary, join(fixture, ".state"));
  const bootstrap = await fetch(server.url, { redirect: "manual" });
  const value = bootstrap.headers.get("set-cookie")?.split(";")[0];
  if (!value) throw new Error("Desktop cookie was not established");
  cookie = value;
  origin = new URL(server.url).origin;
}, 180_000);

afterAll(async () => {
  await server?.stop();
  if (fixture) await rm(fixture, { recursive: true, force: true });
});

describe("packaged native media thumbnails", () => {
  for (const { name, width, height } of [
    { name: "cover.png", width: 224, height: 140 },
    { name: "photo.bmp", width: 224, height: 140 },
    { name: "clip.mp4", width: 224, height: 126 },
    { name: "album.mp3", width: 224, height: 140 },
    { name: "tone.wav", width: 224, height: 144 },
    { name: "document.pdf", width: 224, height: 140 },
  ]) {
    test(`renders ${name} through the private desktop API`, async () => {
      const response = await preview(name);
      expect(response.status).toBe(200);
      expect(response.headers.get("content-type")).toBe("image/jpeg");
      const output = new Uint8Array(await response.arrayBuffer());
      expect(jpegDimensions(output)).toEqual({ width, height });
      expect(response.headers.get("etag")).toBeTruthy();
    }, 15_000);
  }

  test("video seeks the held descriptor even above the image byte limit", async () => {
    await copyFile(join(fixture, "clip.mp4"), join(fixture, "large.mp4"));
    const file = await open(join(fixture, "large.mp4"), "r+");
    await file.truncate(2 * 1024 * 1024 * 1024);
    await file.close();
    expect((await preview("large.mp4")).status).toBe(200);
  }, 15_000);

  test("preserves conditional caching and invalidates replaced files", async () => {
    const first = await preview("cover.png");
    const tag = first.headers.get("etag");
    if (!tag) throw new Error("Missing thumbnail ETag");
    expect((await preview("cover.png", { "If-None-Match": tag })).status).toBe(
      304,
    );
    await writeFile(join(fixture, "cover.png"), "invalid replacement");
    expect((await preview("cover.png", { "If-None-Match": tag })).status).toBe(
      415,
    );
  });

  test("failed or unsupported media returns a controlled icon fallback", async () => {
    expect((await preview("broken.mp4")).status).toBe(415);
    expect((await preview("notes.txt")).status).toBe(415);
    const response = await fetch(
      `${origin}/api/v1/stream/thumbnail/outside/private.mp4`,
      { headers: { Cookie: cookie } },
    );
    expect(response.status).toBe(403);
  });

  test("rejects playlists rather than opening network media", async () => {
    let connections = 0;
    const trap = Bun.serve({
      hostname: "127.0.0.1",
      port: 0,
      fetch: () => {
        connections++;
        return new Response("unexpected");
      },
    });
    try {
      await writeFile(
        join(fixture, "playlist.mp4"),
        `#EXTM3U\n#EXT-X-VERSION:3\n#EXTINF:1,\nhttp://127.0.0.1:${trap.port}/segment.ts\n#EXT-X-ENDLIST\n`,
      );
      expect((await preview("playlist.mp4")).status).toBe(415);
      expect(connections).toBe(0);
    } finally {
      await trap.stop(true);
    }
  });

  test("shutdown cancels active desktop watch streams without killing the backend", async () => {
    const stream = await fetch(
      `${origin}/api/v1/desktop/watch?${new URLSearchParams({ path: `root${fixture}` })}`,
      { headers: { Cookie: cookie } },
    );
    expect(stream.status).toBe(200);
    if (!stream.body) throw new Error("Missing desktop watch stream");
    const reader = stream.body.getReader();
    expect((await reader.read()).done).toBe(false);
    await server.stop();
    expect(await server.exited).toBe(0);
    expect((await reader.read()).done).toBe(true);
  }, 10_000);
});
