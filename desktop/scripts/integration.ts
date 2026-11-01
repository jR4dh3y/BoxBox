import assert from "node:assert/strict";
import {
  access,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { startServer } from "../src/server";

const fixture = await mkdtemp(join(tmpdir(), "boxbox-desktop-integration-"));
const originalHome = process.env.HOME;
const originalData = process.env.XDG_DATA_HOME;
const originalPath = process.env.PATH;
const dataHome = join(fixture, ".local/share");
const bin = join(fixture, "bin");
await mkdir(bin, { recursive: true });
await mkdir(join(dataHome, "applications"), { recursive: true });
await writeFile(
  join(bin, "xdg-open"),
  '#!/bin/sh\nprintf "%s" "$1" > "$HOME/opened-path"\n',
  { mode: 0o700 },
);
await writeFile(
  join(bin, "fixture-viewer"),
  '#!/bin/sh\nprintf "%s" "$1" > "$HOME/chosen-app-path"\n',
  { mode: 0o700 },
);
await writeFile(
  join(dataHome, "applications/boxbox-fixture.desktop"),
  `[Desktop Entry]\nType=Application\nName=BoxBox Fixture Viewer\nExec=${join(bin, "fixture-viewer")} %f\nMimeType=text/plain;\n`,
);
process.env.HOME = fixture;
process.env.XDG_DATA_HOME = dataHome;
process.env.PATH = `${bin}:${originalPath ?? ""}`;
let server: Awaited<ReturnType<typeof startServer>> | undefined;

function trashID(value: unknown): string {
  assert.ok(typeof value === "object" && value !== null && "item" in value);
  const item = value.item;
  assert.ok(
    typeof item === "object" &&
      item !== null &&
      "id" in item &&
      typeof item.id === "string",
  );
  return item.id;
}

async function waitForFile(path: string): Promise<string> {
  for (let attempt = 0; attempt < 50; attempt++) {
    try {
      return await readFile(path, "utf8");
    } catch {
      await Bun.sleep(20);
    }
  }
  throw new Error("Desktop launcher did not produce its capture file");
}

try {
  server = await startServer(
    resolve(import.meta.dir, "../resources/boxbox-server"),
    join(fixture, ".state"),
  );
  const origin = new URL(server.url).origin;
  const bootstrap = await fetch(server.url, { redirect: "manual" });
  const cookie = bootstrap.headers.get("set-cookie")?.split(";")[0];
  assert.ok(cookie);
  const sessionHeaders = { Cookie: cookie };
  async function post(path: string, body: unknown) {
    return fetch(origin + "/api/v1/desktop/" + path, {
      method: "POST",
      headers: { ...sessionHeaders, "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
  }
  const file = join(fixture, "report..txt");
  await writeFile(file, "contents to preserve");
  await writeFile(join(fixture, "report%20.txt"), "literal percent name");
  await writeFile(join(fixture, "report .txt"), "different sibling");
  const clipboardResponse = await post("clipboard", {
    operation: "cut",
    paths: [`root${fixture}/report%20.txt`],
  });
  assert.equal(clipboardResponse.status, 400, await clipboardResponse.clone().text());
  let response = await post("trash", { path: `root${fixture}/report..txt` });
  assert.equal(response.status, 201, await response.clone().text());
  const id = trashID(await response.json());
  await assert.rejects(access(file));
  await writeFile(file, "new occupant");
  response = await post("trash/restore", { id });
  assert.equal(response.status, 409);
  assert.equal(await readFile(file, "utf8"), "new occupant");
  await rm(file);
  assert.equal((await post("trash/restore", { id })).ok, true);
  assert.equal(await readFile(file, "utf8"), "contents to preserve");
  await symlink(file, join(fixture, "link.txt"));
  response = await post("trash", { path: `root${fixture}/link.txt` });
  assert.equal(response.ok, true, await response.clone().text());
  const linkID = trashID(await response.json());
  assert.equal(await readFile(file, "utf8"), "contents to preserve");
  assert.equal((await post("trash/restore", { id: linkID })).ok, true);
  assert.equal(
    await readFile(join(fixture, "link.txt"), "utf8"),
    "contents to preserve",
  );
  response = await post("open", { path: `root${fixture}/report..txt` });
  assert.equal(response.status, 200, await response.clone().text());
  assert.equal(await waitForFile(join(fixture, "opened-path")), file);
  response = await fetch(
    origin + `/api/v1/desktop/apps?path=root${fixture}/report..txt`,
    { headers: { Cookie: cookie } },
  );
  assert.equal(response.status, 200);
  assert.match(await response.text(), /BoxBox Fixture Viewer/);
  response = await post("open-with", {
    path: `root${fixture}/report..txt`,
    appId: "boxbox-fixture.desktop",
  });
  assert.equal(response.ok, true, await response.clone().text());
  assert.equal(await waitForFile(join(fixture, "chosen-app-path")), file);
  response = await fetch(origin + "/api/v1/desktop/volumes", {
    headers: { Cookie: cookie },
  });
  assert.equal(response.status, 200);
  console.log(
    "Desktop integration passed: Trash/restore, no overwrite, symlink safety, dot filenames, default opener, chosen application, volume listing.",
  );
} finally {
  await server?.stop();
  if (originalHome === undefined) delete process.env.HOME;
  else process.env.HOME = originalHome;
  if (originalData === undefined) delete process.env.XDG_DATA_HOME;
  else process.env.XDG_DATA_HOME = originalData;
  if (originalPath === undefined) delete process.env.PATH;
  else process.env.PATH = originalPath;
  await rm(fixture, { recursive: true, force: true });
}
